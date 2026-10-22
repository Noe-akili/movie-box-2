//! Routes API (`/api/*`) : catalogue mondial, sources gratuites et source interne.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::cache::cache;
use crate::config::config;
use crate::error::{ApiError, Result};
use crate::services::archive::{self, ArchiveSearchParams, DEFAULT_ARCHIVE_LICENSE};
use crate::services::commons::{self, CommonsSearchParams};
use crate::services::free::{self, FreeSearchParams};
use crate::services::internal;
use crate::services::open;
use crate::services::providers::{build_legal_links, legal_info};

use crate::services::tmdb::{
	self, cards_from_response, credits_from_response, find_trailer, image_url, is_media_type,
	providers_from_response, resolve_lang,
};

type Params = HashMap<String, String>;

static STARTED_AT: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

fn param<'a>(params: &'a Params, key: &str) -> Option<&'a str> {
	params.get(key).map(|value| value.as_str())
}

fn parse_page(params: &Params) -> i64 {
	params
		.get("page")
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(1)
		.clamp(1, 500)
}

fn parse_rows(params: &Params) -> i64 {
	params
		.get("rows")
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(30)
		.clamp(6, 48)
}


fn lang_of(params: &Params) -> String {
	resolve_lang(param(params, "lang"))
}

fn region_of(params: &Params) -> String {
	param(params, "region")
		.unwrap_or(&config().watch_region)
		.to_uppercase()
		.chars()
		.take(2)
		.collect()
}

fn source_of(params: &Params) -> &'static str {
	free::resolve_free_source(param(params, "source"))
}

fn license_of(params: &Params) -> &'static str {
	let source = source_of(params);
	free::resolve_license(source, param(params, "license"))
}

pub fn router() -> Router {
	Router::new()
		.route("/health", get(health))
		.route("/legal", get(legal))
		.route("/genres", get(genres))
		.route("/home", get(home))
		.route("/search", get(search))
		.route("/discover/{mediaType}", get(discover))
		.route("/free", get(free_titles))
		.route("/title/tv/{id}/season/{seasonNumber}", get(tv_season))
		.route("/title/{mediaType}/{id}", get(title))
		.nest("/downloads", crate::routes::downloads::router())
		.nest("/internal", crate::routes::internal::router())
		.nest("/transfer", crate::routes::transfer::router())
		.fallback(not_found)
}

async fn not_found(request: axum::extract::Request) -> ApiError {
	let method = request.method();
	let path = request.uri();
	ApiError::new(404, format!("Route API inconnue : {method} {path}"))
}

async fn health() -> Json<Value> {
	let (entries, inflight) = cache().stats();
	Json(json!({
		"status": "ok",
		"uptime": STARTED_AT.elapsed().as_secs(),
		"cache": { "entries": entries, "inflight": inflight },
		"tmdbConfigured": crate::config::has_tmdb_key(),
		"region": config().watch_region,
		"runtime": "rust",
		"internal": internal::health().await,
	}))
}

async fn legal() -> Json<Value> {
	Json(legal_info())
}

async fn genres(Query(params): Query<Params>) -> Result<Json<Value>> {
	let kind = if param(&params, "type") == Some("tv") {
		"tv"
	} else {
		"movie"
	};
	let lang = lang_of(&params);
	let payload =
		tmdb::tmdb_request(&format!("/genre/{kind}/list"), &[], config().ttl.long, &lang).await?;

	Ok(Json(json!({
		"type": kind,
		"genres": payload.get("genres").cloned().unwrap_or(json!([])),
	})))
}

/// Rails TMDB de la page d'accueil (le catalogue mondial complet passe par /discover).
struct TmdbRail {
	id: &'static str,
	title: &'static str,
	path: &'static str,
	params: &'static [(&'static str, &'static str)],
	media_type: &'static str,
	href: &'static str,
	ttl: Option<u64>,
}

static TMDB_RAILS: [TmdbRail; 10] = [
	TmdbRail {
		id: "trending-week",
		title: "Tendances de la semaine",
		path: "/trending/all/week",
		params: &[],
		media_type: "movie",
		href: "/movies",
		ttl: None,
	},
	TmdbRail {
		id: "popular-movies",
		title: "Films populaires",
		path: "/movie/popular",
		params: &[],
		media_type: "movie",
		href: "/movies",
		ttl: None,
	},
	TmdbRail {
		id: "popular-tv",
		title: "Séries à ne pas manquer",
		path: "/tv/popular",
		params: &[],
		media_type: "tv",
		href: "/series",
		ttl: None,
	},
	TmdbRail {
		id: "anime",
		title: "Anime & animation japonaise",
		path: "/discover/tv",
		params: &[
			("with_genres", "16"),
			("with_original_language", "ja"),
			("sort_by", "popularity.desc"),
		],
		media_type: "tv",
		href: "/series?genre=16",
		ttl: None,
	},
	TmdbRail {
		id: "top-rated",
		title: "Les mieux notés de tous les temps",
		path: "/movie/top_rated",
		params: &[],
		media_type: "movie",
		href: "/movies?sort=vote_average.desc",
		ttl: Some(6 * 60 * 60 * 1000),
	},
	TmdbRail {
		id: "action",
		title: "Action & aventure",
		path: "/discover/movie",
		params: &[("with_genres", "28"), ("sort_by", "popularity.desc")],
		media_type: "movie",
		href: "/movies?genre=28",
		ttl: None,
	},
	TmdbRail {
		id: "korea",
		title: "K-dramas & cinéma coréen",
		path: "/discover/tv",
		params: &[
			("with_original_language", "ko"),
			("sort_by", "popularity.desc"),
		],
		media_type: "tv",
		href: "/series?language=ko",
		ttl: None,
	},
	TmdbRail {
		id: "french",
		title: "Cinéma français",
		path: "/discover/movie",
		params: &[
			("with_original_language", "fr"),
			("sort_by", "popularity.desc"),
		],
		media_type: "movie",
		href: "/movies?language=fr",
		ttl: None,
	},
	TmdbRail {
		id: "india",
		title: "Bollywood & cinéma indien",
		path: "/discover/movie",
		params: &[
			("with_original_language", "hi"),
			("sort_by", "popularity.desc"),
		],
		media_type: "movie",
		href: "/movies?language=hi",
		ttl: None,
	},
	TmdbRail {
		id: "horror",
		title: "Frissons garantis",
		path: "/discover/movie",
		params: &[("with_genres", "27"), ("sort_by", "popularity.desc")],
		media_type: "movie",
		href: "/movies?genre=27",
		ttl: None,
	},
];

/// Rails de films gratuits et légaux (Internet Archive : domaine public / libre).
struct ArchiveRail {
	id: &'static str,
	title: &'static str,
	collection: &'static str,
	license: &'static str,
}

static ARCHIVE_RAILS: [ArchiveRail; 9] = [
	ArchiveRail {
		id: "free-features",
		title: "Films gratuits — domaine public",
		collection: "feature_films",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-series",
		title: "Séries TV libres de droits",
		collection: "classic_tv",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-noir",
		title: "Polars & cinéma noir",
		collection: "Film_Noir",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-scifi",
		title: "Science-fiction rétro",
		collection: "SciFi_Horror",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-silent",
		title: "Chefs-d'œuvre du cinéma muet",
		collection: "silent_films",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-cartoons",
		title: "Animation & dessins animés",
		collection: "animationandcartoons",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-shorts",
		title: "Courts-métrages libres",
		collection: "short_films",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-prelinger",
		title: "Archives Prelinger (1950-1980)",
		collection: "prelinger",
		license: DEFAULT_ARCHIVE_LICENSE,
	},
	ArchiveRail {
		id: "free-cc-modern",
		title: "Films sous Creative Commons",
		collection: "feature_films",
		license: "cc",
	},
];

enum HomeMsg {
	Tmdb { index: usize, payload: Option<Value> },
	Archive(Vec<Option<Value>>),
	Commons(Option<Value>),
	Open(Vec<Value>),
	Internal(std::result::Result<Vec<Value>, ()>),
}

/// Page d'accueil : toutes les sources démarrent en même temps (budget global).
async fn home(Query(params): Query<Params>) -> Result<Json<Value>> {
	let lang = lang_of(&params);
	let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HomeMsg>();

	// 1. Rails TMDB (chaque rail répond à son rythme, une erreur ne casse pas les suivants).
	{
		let tx = tx.clone();
		let lang = lang.clone();
		tokio::spawn(async move {
			for (index, rail) in TMDB_RAILS.iter().enumerate() {
				let ttl = rail.ttl.unwrap_or(config().ttl.short);
				let rail_params: Vec<(String, String)> = rail
					.params
					.iter()
					.map(|(key, value)| (key.to_string(), value.to_string()))
					.collect();
				let borrowed: Vec<(&str, String)> = rail_params
					.iter()
					.map(|(key, value)| (key.as_str(), value.clone()))
					.collect();

				let payload = match tmdb::tmdb_request(rail.path, &borrowed, ttl, &lang).await {
					Ok(value) => Some((*value).clone()),
					Err(error) => {
						eprintln!("[movie-box] rail {} indisponible : {}", rail.id, error);
						None
					}
				};
				let _ = tx.send(HomeMsg::Tmdb { index, payload });
			}
		});
	}

	// 2. Rails Archive.org (4 recherches simultanées maximum).
	{
		let tx = tx.clone();
		tokio::spawn(async move {
			let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
			let tasks = ARCHIVE_RAILS.iter().map(|rail| {
				let semaphore = semaphore.clone();
				async move {
					let _permit = semaphore.acquire().await;
					archive::search_archive(ArchiveSearchParams {
						q: "",
						collection: rail.collection,
						license: rail.license,
						page: 1,
						rows: 20,
						sort: Some("popular"),
						timeout_ms: 10_000,
						retries: 1,
					})
					.await
				}
			});
			let results = futures::future::join_all(tasks).await;
			let mapped: Vec<Option<Value>> = results
				.into_iter()
				.map(|result| match result {
					Ok(value) => Some((*value).clone()),
					Err(error) => {
						eprintln!("[movie-box] rail archive indisponible : {error}");
						None
					}
				})
				.collect();
			let _ = tx.send(HomeMsg::Archive(mapped));
		});
	}

	// 3. Rail Wikimedia Commons.
	{
		let tx = tx.clone();
		tokio::spawn(async move {
			let result = commons::search_commons(CommonsSearchParams {
				q: "",
				category: "",
				page: 1,
				rows: 20,
			})
			.await;
			let payload = match result {
				Ok(value) => Some((*value).clone()),
				Err(error) => {
					eprintln!("[movie-box] rail Wikimedia indisponible : {error}");
					None
				}
			};
			let _ = tx.send(HomeMsg::Commons(payload));
		});
	}

	// 4. Rails des catalogues libres supplémentaires (NASA, Blender, …).
	{
		let tx = tx.clone();
		tokio::spawn(async move {
			let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(3));
			let tasks = open::enabled_sources().into_iter().map(|source| {
				let semaphore = semaphore.clone();
				async move {
					let _permit = semaphore.acquire().await;
					let result = free::search_free(FreeSearchParams {
						source,
						q: "",
						collection: free::default_free_collection(source),
						license: "free",
						page: 1,
						rows: 12,
						sort: Some("popular"),
					})
					.await;
					match result {
						Ok(value) => Some((source, (*value).clone())),
						Err(error) => {
							eprintln!("[movie-box] rail {source} indisponible : {error}");
							None
						}
					}
				}
			});
			let results = futures::future::join_all(tasks).await;
			let rails: Vec<Value> = results
				.into_iter()
				.flatten()
				.filter_map(|(source, payload)| {
					let items = payload
						.get("items")
						.and_then(Value::as_array)
						.cloned()
						.unwrap_or_default();
					if items.is_empty() {
						return None;
					}
					Some(json!({
						"id": format!("free-{source}"),
						"title": open::label_for_source(source),
						"kind": "archive",
						"href": format!("/free?source={source}"),
						"items": items,
					}))
				})
				.collect();
			let _ = tx.send(HomeMsg::Open(rails));
		});
	}

	// 5. Rail de la source interne (moviebox-internal-api).
	{
		let tx = tx.clone();
		tokio::spawn(async move {
			let result = internal::home_cards().await.map_err(|_| ());
			let _ = tx.send(HomeMsg::Internal(result));
		});
	}
	drop(tx);

	#[derive(Default)]
	struct HomeState {
		tmdb: Vec<Option<Value>>,
		tmdb_count: usize,
		archive: Option<Vec<Option<Value>>>,
		common: Option<Option<Value>>,
		open: Option<Vec<Value>>,
		internal: Option<Result<Vec<Value>, ()>>,
	}

	impl HomeState {
		fn apply(&mut self, message: HomeMsg) {
			match message {
				HomeMsg::Tmdb { index, payload } => {
					if self.tmdb[index].is_none() {
						self.tmdb_count += 1;
					}
					self.tmdb[index] = payload;
				}
				HomeMsg::Archive(results) => self.archive = Some(results),
				HomeMsg::Commons(payload) => self.common = Some(payload),
				HomeMsg::Open(rails) => self.open = Some(rails),
				HomeMsg::Internal(result) => self.internal = Some(result),
			}
		}

		fn complete(&self) -> bool {
			self.tmdb_count >= TMDB_RAILS.len()
				&& self.archive.is_some()
				&& self.common.is_some()
				&& self.internal.is_some()
		}
	}

	let mut state = HomeState {
		tmdb: vec![None; TMDB_RAILS.len()],
		..Default::default()
	};

	let deadline = tokio::time::sleep(std::time::Duration::from_millis(config().home_deadline_ms));
	tokio::pin!(deadline);

	loop {
		if state.complete() {
			break;
		}
		tokio::select! {
			_ = &mut deadline => break,
			message = rx.recv() => match message {
				Some(message) => state.apply(message),
				None => break,
			},
		}
	}

	// Les messages déjà prêts sont récupérés même si le budget est épuisé.
	while let Ok(message) = rx.try_recv() {
		state.apply(message);
	}

	let pending = (state.tmdb_count < TMDB_RAILS.len()) as i32
		+ state.archive.is_none() as i32
		+ state.common.is_none() as i32
		+ state.open.is_none() as i32
		+ state.internal.is_none() as i32;

	let mut rails: Vec<Value> = Vec::new();
	let mut hero: Vec<Value> = Vec::new();

	for (index, payload) in state.tmdb.iter().enumerate() {
		let rail = &TMDB_RAILS[index];
		let Some(payload) = payload else { continue };
		let items = cards_from_response(payload, rail.media_type);
		if items.is_empty() {
			continue;
		}
		if rail.id == "trending-week" {
			hero = items
				.iter()
				.filter(|item| item.get("backdrop").is_some_and(|value| !value.is_null()))
				.cloned()
				.take(8)
				.collect();
		}
		rails.push(json!({
			"id": rail.id,
			"title": rail.title,
			"kind": "tmdb",
			"href": rail.href,
			"items": items,
		}));
	}

	if let Some(results) = state.archive {
		for (index, result) in results.into_iter().enumerate() {
			let rail = &ARCHIVE_RAILS[index];
			let Some(result) = result else { continue };
			let items = result
				.get("items")
				.and_then(|value| value.as_array())
				.cloned()
				.unwrap_or_default();
			if items.is_empty() {
				continue;
			}
			let mut href = format!("/free?collection={}", rail.collection);
			if rail.license != DEFAULT_ARCHIVE_LICENSE {
				href.push_str(&format!("&license={}", rail.license));
			}
			rails.push(json!({
				"id": rail.id,
				"title": rail.title,
				"kind": "archive",
				"href": href,
				"items": items,
			}));
		}
	}

	if let Some(Some(result)) = state.common {
		let items = result
			.get("items")
			.and_then(|value| value.as_array())
			.cloned()
			.unwrap_or_default();
		if !items.is_empty() {
			rails.push(json!({
				"id": "free-commons",
				"title": "Domaine public — Wikimedia Commons",
				"kind": "archive",
				"href": "/free?source=commons",
				"items": items,
			}));
		}
	}

	if let Some(open_rails) = state.open {
		rails.extend(open_rails);
	}

	if let Some(Ok(cards)) = state.internal {
		if !cards.is_empty() {
			// Source principale : son rail ouvre la page d'accueil.
			rails.insert(
				0,
				json!({
					"id": "internal-home",
					"title": "MovieBox — films & séries",
					"kind": "internal",
					"href": "/free?source=internal",
					"items": cards,
				}),
			);
		}
	}

	if hero.is_empty() {
		hero = rails
			.iter()
			.flat_map(|rail| {
				rail.get("items")
					.and_then(|value| value.as_array())
					.cloned()
					.unwrap_or_default()
			})
			.filter(|item| item.get("backdrop").is_some_and(|value| !value.is_null()))
			.take(8)
			.collect();
	}

	Ok(Json(json!({
		"lang": lang,
		"region": config().watch_region,
		"hero": hero,
		"rails": rails,
		// Vrai quand une source n'a pas répondu dans le budget : l'interface propose de réessayer.
		"partial": pending > 0,
		"generatedAt": chrono::Utc::now().to_rfc3339(),
	})))
}

static SORT_WHITELIST: [&str; 7] = [
	"popularity.desc",
	"popularity.asc",
	"vote_average.desc",
	"vote_count.desc",
	"primary_release_date.desc",
	"primary_release_date.asc",
	"revenue.desc",
];

/// Recherche globale : TMDB + sources gratuites (Archive / Commons / interne).
async fn search(Query(params): Query<Params>) -> Result<Json<Value>> {
	let query = param(&params, "q").unwrap_or("").trim().to_string();
	if query.is_empty() {
		return Err(ApiError::bad_request("Paramètre « q » requis"));
	}

	let page = parse_page(&params);
	let lang = lang_of(&params);
	let kind = match param(&params, "type") {
		Some("movie") => "movie",
		Some("tv") => "tv",
		_ => "all",
	};
	let year = param(&params, "year")
		.filter(|value| value.len() == 4 && value.chars().all(|c| c.is_ascii_digit()))
		.map(|value| value.to_string());

	let tmdb_paths: Vec<&str> = match kind {
		"movie" => vec!["/search/movie"],
		"tv" => vec!["/search/tv"],
		_ => vec!["/search/movie", "/search/tv"],
	};

	let source = source_of(&params);
	let license = license_of(&params);

	let tmdb_task = async {
		let mut settled: Vec<Result<std::sync::Arc<Value>>> = Vec::new();
		for path in &tmdb_paths {
			let mut query_params: Vec<(String, String)> = vec![
				("query".to_string(), query.clone()),
				("page".to_string(), page.to_string()),
				("include_adult".to_string(), "false".to_string()),
			];
			if let Some(year) = &year {
				query_params.push(("year".to_string(), year.clone()));
			}
			let borrowed: Vec<(&str, String)> = query_params
				.iter()
				.map(|(key, value)| (key.as_str(), value.clone()))
				.collect();
			settled.push(tmdb::tmdb_request(path, &borrowed, config().ttl.short, &lang).await);
		}
		settled
	};

	let free_task = free::search_free(FreeSearchParams {
		source,
		q: &query,
		collection: "",
		license,
		page,
		rows: 20,
		sort: Some("popular"),
	});

	// La recherche globale couvre les deux catalogues libres, quel que soit l'onglet choisi.
	let commons_task = async {
		if source == "archive" {
			commons::search_commons(CommonsSearchParams {
				q: &query,
				category: "",
				page,
				rows: 12,
			})
			.await
			.map(Some)
		} else {
			Ok(None)
		}
	};

	// Les catalogues libres supplémentaires complètent la recherche, sauf
	// celui qui est déjà la source principale.
	let open_task = async {
		let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(3));
		let tasks = open::enabled_sources()
			.into_iter()
			.filter(|open_source| *open_source != source)
			.map(|open_source| {
				let semaphore = semaphore.clone();
				let query = query.clone();
				async move {
					let _permit = semaphore.acquire().await;
					open::search(open::OpenSearchParams {
						source: open_source,
						q: &query,
						collection: "",
						page,
						rows: 6,
						sort: Some("popular"),
					})
					.await
					.ok()
				}
			});
		let results = futures::future::join_all(tasks).await;
		let mut items: Vec<Value> = Vec::new();
		for result in results.into_iter().flatten() {
			if let Some(list) = result.get("items").and_then(Value::as_array) {
				items.extend(list.iter().cloned());
			}
		}
		items
	};

	let (tmdb_settled, free_settled, commons_settled, open_items) =
		tokio::join!(tmdb_task, free_task, commons_task, open_task);

	let results: Vec<Value> = if tmdb_settled.iter().all(|result| result.is_ok()) {
		let mut cards: Vec<Value> = tmdb_settled
			.iter()
			.enumerate()
			.flat_map(|(index, result)| {
				let fallback = if tmdb_paths[index] == "/search/tv" {
					"tv"
				} else {
					"movie"
				};
				match result {
					Ok(payload) => cards_from_response(payload, fallback),
					Err(_) => vec![],
				}
			})
			.collect();
		cards.sort_by(|a, b| {
			let popularity = |item: &Value| {
				item.get("popularity")
					.and_then(|value| value.as_f64())
					.unwrap_or(0.0)
			};
			popularity(b)
				.partial_cmp(&popularity(a))
				.unwrap_or(std::cmp::Ordering::Equal)
		});
		cards
	} else {
		vec![]
	};
		let count = results.len();

		let primary_free: std::sync::Arc<Value> = free_settled.unwrap_or_else(|_| {
		Arc::new(json!({
			"source": source,
			"license": license,
			"total": 0,
			"hasMore": false,
			"items": [],
		}))
	});
	let primary_free = primary_free.as_ref();
	let known_ids: Vec<String> = primary_free
		.get("items")
		.and_then(|value| value.as_array())
		.map(|items| {
			items
				.iter()
				.filter_map(|item| item.get("archiveId").and_then(|value| value.as_str()))
				.map(|value| value.to_string())
				.collect()
		})
		.unwrap_or_default();

	let commons_owned = commons_settled.ok().flatten();
	let commons_free: Option<&Value> = commons_owned.as_ref().map(|arc| arc.as_ref());
	let mut free_items: Vec<Value> = primary_free
		.get("items")
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();
	let mut commons_count = 0;
	let mut seen_ids: Vec<String> = known_ids.clone();
	if let Some(commons_result) = commons_free {
		let commons_items = commons_result
			.get("items")
			.and_then(|value| value.as_array())
			.cloned()
			.unwrap_or_default();
		commons_count = commons_items.len();
		for item in commons_items {
			let archive_id = item.get("archiveId").and_then(|value| value.as_str());
			if archive_id.is_some_and(|id| seen_ids.contains(&id.to_string())) {
				continue;
			}
			if let Some(id) = archive_id {
				seen_ids.push(id.to_string());
			}
			free_items.push(item);
		}
	}

	let open_count = open_items.len();
	for item in open_items {
		let archive_id = item.get("archiveId").and_then(|value| value.as_str());
		if archive_id.is_some_and(|id| seen_ids.contains(&id.to_string())) {
			continue;
		}
		if let Some(id) = archive_id {
			seen_ids.push(id.to_string());
		}
		free_items.push(item);
	}

	Ok(Json(json!({
		"query": query,
		"page": page,
		"type": kind,
		"count": count,
		"results": results,
		"free": {
			"source": source,
			"license": primary_free.get("license").cloned().unwrap_or(Value::String(license.to_string())),
			"total": primary_free.get("total").cloned().unwrap_or(Value::from(0)),
			"hasMore": primary_free.get("hasMore").cloned().unwrap_or(Value::Bool(false)),
			"commons": commons_count,
			"open": open_count,
			"items": free_items,
		},
	})))
}

/// Catalogue mondial filtré (TMDB discover).
async fn discover(
	Path(media_type): Path<String>,
	Query(params): Query<Params>,
) -> Result<Json<Value>> {
	if !is_media_type(&media_type) {
		return Err(ApiError::bad_request("Type de média invalide (movie ou tv)"));
	}

	let lang = lang_of(&params);
	let page = parse_page(&params);
	let sort = param(&params, "sort")
		.filter(|value| SORT_WHITELIST.contains(value))
		.unwrap_or("popularity.desc")
		.to_string();
	let genre: String = param(&params, "genre")
		.unwrap_or("")
		.chars()
		.filter(|c| c.is_ascii_digit() || *c == '|')
		.collect();
	let language: Option<String> = param(&params, "language")
		.filter(|value| value.len() == 2 && value.chars().all(|c| c.is_ascii_lowercase()))
		.map(|value| value.to_string());
	let country: Option<String> = {
		let raw: String = param(&params, "country")
			.unwrap_or("")
			.chars()
			.filter(|c| c.is_ascii_alphabetic() || *c == '|')
			.collect();
		let upper = raw.to_uppercase();
		if upper.is_empty() {
			None
		} else {
			Some(upper)
		}
	};
	let year: Option<String> = param(&params, "year")
		.filter(|value| value.len() == 4 && value.chars().all(|c| c.is_ascii_digit()))
		.map(|value| value.to_string());

	let mut query_params: Vec<(String, String)> = vec![
		("page".to_string(), page.to_string()),
				("sort_by".to_string(), sort.clone()),
		("include_adult".to_string(), "false".to_string()),
	];
	if !genre.is_empty() {
		query_params.push(("with_genres".to_string(), genre));
	}
	if let Some(language) = language {
		query_params.push(("with_original_language".to_string(), language));
	}
	if let Some(country) = country {
		query_params.push(("with_origin_country".to_string(), country));
	}
	if let Some(year) = year {
		let key = if media_type == "tv" {
			"first_air_date_year"
		} else {
			"primary_release_year"
		};
		query_params.push((key.to_string(), year));
	}
	// Tri par note exige une popularité minimale (équivalent à JS server/routes/api.js).
	if sort.starts_with("vote_average") {
		query_params.push(("vote_count.gte".to_string(), "120".to_string()));
	}

	let borrowed: Vec<(&str, String)> = query_params
		.iter()
		.map(|(key, value)| (key.as_str(), value.clone()))
		.collect();
	let payload = tmdb::tmdb_request(
		&format!("/discover/{media_type}"),
		&borrowed,
		config().ttl.short,
		&lang,
	)
	.await?;

	let tmdb_page = payload
		.get("page")
		.and_then(|value| value.as_i64())
		.unwrap_or(page);
	let total_pages = payload
		.get("total_pages")
		.and_then(|value| value.as_i64())
		.unwrap_or(1)
		.min(500);
	let total = payload
		.get("total_results")
		.and_then(|value| value.as_i64())
		.unwrap_or(0);

		Ok(Json(json!({
		"mediaType": media_type,
		"page": tmdb_page,
		"totalPages": total_pages,
		"total": total,
		"hasMore": tmdb_page < total_pages,
		"items": cards_from_response(&payload, &media_type),
	})))
}

/// Titres provenant des sources gratuites (Archive / Commons / interne).
async fn free_titles(Query(params): Query<Params>) -> Result<Json<Value>> {
	let source = source_of(&params);
	let collection: String = param(&params, "collection").unwrap_or("").to_string();
	let license = license_of(&params);
	let q: String = param(&params, "q").unwrap_or("").to_string();
	let page = parse_page(&params);
		let rows = parse_rows(&params);

	let allowed_sort = ["popular", "recent", "oldest", "title", "rating"];
	let sort = param(&params, "sort")
		.filter(|value| allowed_sort.contains(value))
		.unwrap_or("popular")
		.to_string();

	let collections = free::free_collections(source);
	let collection = if collections
		.as_array()
		.map(|items| {
			items
				.iter()
				.any(|item| item.get("id").and_then(|value| value.as_str()) == Some(&collection))
		})
		.unwrap_or(false)
	{
		collection.clone()
	} else {
		free::default_free_collection(source).to_string()
	};

	let search_params = FreeSearchParams {
		source,
		q: &q,
		collection: &collection,
		license,
		page,
		rows,
		sort: Some(&sort),
	};

	let result: Value = if q.is_empty() && collection.is_empty() {
		json!({
			"source": source,
			"license": license,
			"total": 0,
			"hasMore": false,
			"items": [],
		})
	} else {
		free::search_free(search_params)
			.await
			.map(|arc| (*arc).clone())
			.unwrap_or_else(|_| {
				json!({
					"source": source,
					"license": license,
					"total": 0,
					"hasMore": false,
					"items": [],
				})
			})
	};

	let mut result_obj: serde_json::Map<String, Value> =
		result.as_object().cloned().unwrap_or_default();
	result_obj
		.entry("source".to_string())
		.or_insert_with(|| Value::String(source.to_string()));
	result_obj
		.entry("collection".to_string())
		.or_insert_with(|| Value::String(collection.clone()));
	result_obj
		.entry("sort".to_string())
		.or_insert_with(|| Value::String(sort.clone()));
	result_obj
		.entry("license".to_string())
		.or_insert_with(|| Value::String(license.to_string()));
	result_obj.entry("collections".to_string()).or_insert_with(|| collections);
	result_obj
		.entry("licenses".to_string())
		.or_insert_with(|| free::free_licenses(source));
	result_obj.entry("sources".to_string()).or_insert_with(|| free::free_sources());
	result_obj
		.entry("legal".to_string())
		.or_insert_with(|| legal_info());

	Ok(Json(Value::Object(result_obj)))
}



/// Fiche détaillée d'un titre.
///
/// - `archive`, `commons`, `internal` : délégué directement à la source gratuite concernée.
/// - `movie`, `tv` : fiche TMDB complète (append + fallback langue) agrémentée de liens légaux.
async fn title(
	Path((media_type, id)): Path<(String, String)>,
	Query(params): Query<Params>,
) -> Result<Json<Value>> {
	// Sources gratuites : lecture directe, contrat préservé par le service concerné.
	if matches!(media_type.as_str(), "archive" | "commons" | "internal") {
		let free_title = free::get_free_title_from(&id, Some(media_type.as_str())).await?;
		return Ok(Json(free_title));
	}

	if !is_media_type(&media_type) {
		return Err(ApiError::bad_request("Type de média invalide (movie, tv, archive, …)"));
	}

	let lang = lang_of(&params);
	let region = region_of(&params);
	let raw_id: String = id.chars().filter(|c| c.is_ascii_digit()).collect();
	let tmdb_id: u32 = raw_id
		.parse()
		.map_err(|_| ApiError::bad_request("Identifiant TMDB invalide"))?;

	// Append complet : TMDB renvoie credits / videos / images / recommandations / similar / providers.
	let append = "credits,videos,images,recommendations,similar,watch/providers";
	let append = if media_type == "tv" {
		format! {"{append},external_ids,content_ratings"}
	} else {
		format! {"{append},release_dates"}
	};
	let query_params: Vec<(String, String)> = vec![
		("append_to_response".to_string(), append.clone()),
		("language".to_string(), resolve_lang(Some(&lang))),
		("watch_region".to_string(), region.clone()),
		(
			"include_image_language".to_string(),
			format! {"{}/en/null", lang.chars().take(2).collect::<String>()},
		),
	];
	let borrowed: Vec<(&str, String)> = query_params
		.iter()
		.map(|(key, value)| (key.as_str(), value.clone()))
		.collect();

	let mut data = tmdb::tmdb_request(
		&format!("/{media_type}/{tmdb_id}"),
		&borrowed,
		config().ttl.medium,
		&lang,
	)
	.await?
	.as_ref()
	.clone();

	// TMDB ne fournit pas toujours de synopsis traduit : repli sur l'anglais.
	if data.get("overview").and_then(|value| value.as_str()).is_none() && lang != "en-US" {
		let fallback_params: Vec<(String, String)> = vec![
			("append_to_response".to_string(), append),
			("language".to_string(), "en-US".to_string()),
			("watch_region".to_string(), region.clone()),
			(
				"include_image_language".to_string(),
				format! {"{0}/en/null", lang.chars().take(2).collect::<String>()},
			),
		];
		let borrowed_fb: Vec<(&str, String)> = fallback_params
			.iter()
			.map(|(key, value)| (key.as_str(), value.clone()))
			.collect();
		if let Ok(value) = tmdb::tmdb_request(
			&format!("/{media_type}/{tmdb_id}"),
			&borrowed_fb,
			config().ttl.medium,
			&lang,
		)
		.await
		{
			if value.get("overview").and_then(|value| value.as_str()).is_some() {
				if let Some(overview) = value.get("overview").cloned() {
					data["overview"] = overview;
				}
			}
		}
	}

	let card = tmdb::card_from_tmdb(&data, &media_type)
		.ok_or_else(|| ApiError::not_found("Ce titre n'existe pas dans le catalogue TMDB"))?;
	let title_text = card
		.get("title")
		.and_then(|value| value.as_str())
		.unwrap_or("")
		.to_string();
	let year = card.get("year").and_then(|value| value.as_i64());

	let credits = credits_from_response(data.get("credits").unwrap_or(&Value::Null));
	let similar_source: Vec<&Value> = match data.get("recommendations") {
		Some(value) => value
			.get("results")
			.and_then(|value| value.as_array())
			.map(|items| items.iter().collect())
			.unwrap_or_default(),
		None => match data.get("similar") {
			Some(value) => value
				.get("results")
				.and_then(|value| value.as_array())
				.map(|items| items.iter().collect())
				.unwrap_or_default(),
			None => vec![],
		},
	};
	let similar: Vec<Value> = similar_source
		.iter()
		.filter_map(|item| tmdb::card_from_tmdb(item, &media_type))
				.take(18)
		.collect();

	let age_rating = if media_type == "tv" {
		data.get("content_ratings")
			.and_then(|value| value.get("results"))
			.and_then(|value| value.as_array())
			.and_then(|items| {
				items.iter().find(|item| {
					item.get("iso_3166_1").and_then(|value| value.as_str()) == Some(&region)
				})
			})
			.and_then(|item| item.get("rating").and_then(|value| value.as_str()))
			.map(Value::from)
			.unwrap_or(Value::Null)
	} else {
		data.get("release_dates")
			.and_then(|value| value.get("results"))
			.and_then(|value| value.as_array())
			.and_then(|items| {
				items.iter().find(|item| {
					item.get("iso_3166_1").and_then(|value| value.as_str()) == Some(&region)
				})
			})
			.and_then(|item| {
				item.get("release_dates")
					.and_then(|value| value.as_array())
					.and_then(|dates| dates.first())
					.and_then(|date| date.get("certification"))
			})
			.and_then(|value| value.as_str())
			.map(Value::from)
			.unwrap_or(Value::Null)
	};

	let imdb_id = data
		.get("imdb_id")
		.and_then(|value| value.as_str())
		.filter(|value| !value.is_empty())
		.or_else(|| {
			data.get("external_ids")
				.and_then(|value| value.get("imdb_id"))
				.and_then(|value| value.as_str())
		})
		.map(Value::from)
		.unwrap_or(Value::Null);

	let card_value: Value = serde_json::to_value(&card)
		.map_err(|error| ApiError::new(500, format!("Erreur de sérialisation : {error}")))?;

		let mut result = json!({
		"playable": false,
		"tagline": data.get("tagline").cloned().unwrap_or(Value::Null),
		"status": data.get("status").cloned().unwrap_or(Value::Null),
		"homepage": data.get("homepage").cloned().unwrap_or(Value::Null),
		"networks": data
			.get("networks")
			.and_then(|value| value.as_array())
			.map(|items| {
				items.iter().filter_map(|item| item.get("name").cloned()).collect::<Vec<_>>()
			})
			.unwrap_or_default(),
		"studios": data
			.get("production_companies")
			.and_then(|value| value.as_array())
			.map(|items| {
				items
					.iter()
					.filter_map(|item| item.get("name").cloned())
					.take(4)
					.collect::<Vec<_>>()
			})
			.unwrap_or_default(),
		"countries": data
			.get("production_countries")
			.and_then(|value| value.as_array())
			.map(|items| {
				items.iter().filter_map(|item| item.get("name").cloned()).collect::<Vec<_>>()
			})
			.unwrap_or_default(),
		"spokenLanguages": data
			.get("spoken_languages")
			.and_then(|value| value.as_array())
			.map(|items| {
				items
					.iter()
					.filter_map(|item| {
						item.get("english_name").cloned().or_else(|| item.get("name").cloned())
					})
					.collect::<Vec<_>>()
			})
			.unwrap_or_default(),
		"budget": data.get("budget").cloned().unwrap_or(Value::Null),
		"revenue": data.get("revenue").cloned().unwrap_or(Value::Null),
		"ageRating": age_rating,
		"credits": credits,
		"trailer": find_trailer(
			data.get("videos")
				.and_then(|value| value.get("results"))
				.unwrap_or(&Value::Null),
		),
		"providers": providers_from_response(
			data.get("watch/providers").unwrap_or(&Value::Null),
			&region,
		),
		"legalLinks": build_legal_links(&title_text, year, &media_type, Some(tmdb_id as i64)),
		"screenshots": data
			.get("images")
			.and_then(|value| value.get("backdrops"))
			.and_then(|value| value.as_array())
			.map(|items| {
				items
					.iter()
					.filter_map(|item| {
						image_url(
							item.get("file_path").and_then(|value| value.as_str()),
							"w780",
						)
						.map(Value::String)
					})
					.take(8)
					.collect::<Vec<_>>()
			})
			.unwrap_or_default(),
		"seasons": if media_type == "tv" {
			data.get("seasons")
				.and_then(|value| value.as_array())
				.map(|items| {
					items
						.iter()
						.filter_map(|season| {
							let number = season
								.get("season_number")
								.and_then(|value| value.as_i64())?;
							if number <= 0 {
								return None;
							}
							Some(json!({
								"seasonNumber": number,
								"name": season.get("name").cloned().unwrap_or(Value::Null),
								"episodeCount": season.get("episode_count").cloned().unwrap_or(Value::Null),
								"airDate": season.get("air_date").cloned().unwrap_or(Value::Null),
								"poster": image_url(
									season.get("poster_path").and_then(|value| value.as_str()),
									"w342",
								)
								.map(Value::String)
								.unwrap_or(Value::Null),
								"overview": season
									.get("overview")
									.and_then(|value| value.as_str())
									.unwrap_or("")
									.to_string(),
							}))
						})
						.collect::<Vec<_>>()
				})
				.unwrap_or_default()
		} else {
			vec![]
		},
		"similar": similar,
		"imdbId": imdb_id,
				"legal": legal_info(),
	});

	let card_obj = card_value.as_object().cloned().unwrap_or_default();
	if let Value::Object(base) = &mut result {
		for (key, value) in card_obj {
			base.entry(key).or_insert(value);
		}
	}

		Ok(Json(result))
}

/// Épisodes d'une saison de série (fiche TMDB complète, conformément à l'API Node).
async fn tv_season(
	Path((id, season, _episode)): Path<(String, String, String)>,
	Query(params): Query<Params>,
) -> Result<Json<Value>> {
	let tmdb_id: u32 = id
		.parse()
		.map_err(|_| ApiError::bad_request("Identifiant de série invalide"))?;
	let season_number: u32 = season
		.parse()
		.map_err(|_| ApiError::bad_request("Numéro de saison invalide"))?;
	let lang = lang_of(&params);

	let payload = tmdb::tmdb_request(
		&format! {"/tv/{tmdb_id}/season/{season_number}"},
		&[],
		config().ttl.medium,
		&lang,
	)
	.await?;

	let episodes: Vec<Value> = payload
		.get("episodes")
		.and_then(|value| value.as_array())
		.map(|items| {
			items
				.iter()
				.map(|episode| {
					let episode_number = episode
						.get("episode_number")
						.and_then(|value| value.as_i64())
						.unwrap_or(0);
					json!({
						"id": episode.get("id").cloned().unwrap_or(Value::Null),
						"episodeNumber": episode_number,
						"name": episode
							.get("name")
							.and_then(|value| value.as_str())
							.filter(|value| !value.is_empty())
							.map(Value::from)
							.unwrap_or_else(|| json!(format! {"Épisode {episode_number}"})),
						"overview": episode
							.get("overview")
							.and_then(|value| value.as_str())
							.unwrap_or("")
							.to_string(),
						"still": tmdb::image_url(
							episode.get("still_path").and_then(|value| value.as_str()),
							"w500",
						)
						.map(Value::String)
						.unwrap_or(Value::Null),
						"airDate": episode.get("air_date").cloned().unwrap_or(Value::Null),
						"runtime": episode.get("runtime").cloned().unwrap_or(Value::Null),
						"rating": episode
							.get("vote_average")
							.and_then(|value| value.as_f64())
							.map(|value| Value::from(tmdb::round1(value))),
					})
				})
				.collect()
		})
		.unwrap_or_default();

	Ok(Json(json!({
		"seasonNumber": season_number,
		"name": payload
			.get("name")
			.and_then(|value| value.as_str())
			.unwrap_or(&format! {"Saison {season_number}"}.to_string())
			.to_string(),
		"overview": payload
			.get("overview")
			.and_then(|value| value.as_str())
			.unwrap_or("")
			.to_string(),
		"poster": tmdb::image_url(
			payload.get("poster_path").and_then(|value| value.as_str()),
			"w500",
		)
		.map(Value::String)
		.unwrap_or(Value::Null),
		"airDate": payload.get("air_date").cloned().unwrap_or(Value::Null),
		"episodes": episodes,
	})))
}

