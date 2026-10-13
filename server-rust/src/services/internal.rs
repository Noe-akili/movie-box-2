//! Client de l'API interne `moviebox-internal-api` (FastAPI, défaut 127.0.0.1:8000).
//!
//! Convertit les réponses de l'API interne (films / séries / flux CDN) vers le contrat
//! de données Movie Box : cartes, fiches jouables et flux relus via un proxy local
//! (`/api/internal/proxy`) qui contourne les contraintes CORS du navigateur.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::cache::cache;
use crate::config::config;
use crate::error::{ApiError, Result};
use crate::http::{build_url, fetch_json};

use super::archive::{first_value_str, strip_html};
use super::providers::{build_legal_links, urlencode};
use super::tmdb::{credits_from_response, round1};

pub const ID_PREFIX: &str = "internal:";
pub const SOURCE_LABEL: &str = "MovieBox — source principale";

/// Collections exposées (une par rubrique de l'API interne).
pub const COLLECTIONS: [(&str, &str); 10] = [
	("home", "Accueil"),
	("movies", "Films"),
	("anime", "Anime"),
	("trending", "Tendances"),
	("nollywood", "Nollywood"),
	("asian", "Cinéma asiatique"),
	("western", "Hollywood"),
	("kids", "Jeunesse"),
	("music", "Musique"),
	("education", "Éducation"),
];

pub const DEFAULT_COLLECTION: &str = "home";

pub fn enabled() -> bool {
	config().internal_api_enabled
}

/// Appel JSON vers l'API interne avec cache court.
async fn internal_fetch(path: &str, query: &[(&str, String)], ttl_ms: u64) -> Result<Value> {
	if !enabled() {
		return Err(ApiError::new(
			503,
			"Source interne désactivée (INTERNAL_API_ENABLED=false)",
		));
	}

	let url = build_url(&format!("{}{}", config().internal_api_url, path), query);
	let key = format!("internal:{url}");

	let result = cache()
		.cached(&key, ttl_ms, || async {
			let value = fetch_json(&url, 20_000, 1, &[]).await?;
			Ok(Arc::new(value))
		})
		.await;

	result.map(|value| (*value).clone()).map_err(|_| {
			ApiError::new(
				503,
				"API interne moviebox-internal-api indisponible (lance « python moviebox_api_server.py »)",
			)
		})
}

/// Extrait la liste d'éléments d'une réponse hétérogène (`data.list`, `data.items`, tableau…).
pub fn extract_items(payload: &Value) -> Vec<Value> {
	const KEYS: [&str; 6] = ["list", "items", "subjects", "movie", "results", "data"];

	if let Value::Array(items) = payload {
		return items.clone();
	}

	if let Some(data) = payload.get("data") {
		if let Value::Array(items) = data {
			return items.clone();
		}
		for key in KEYS {
			if let Some(Value::Array(items)) = data.get(key) {
				return items.clone();
			}
		}
	}

	for key in KEYS {
		if let Some(Value::Array(items)) = payload.get(key) {
			return items.clone();
		}
	}

	vec![]
}

fn as_str(value: Option<&Value>) -> Option<String> {
	match value? {
		Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
		Value::Number(number) => Some(number.to_string()),
		_ => None,
	}
}

fn first_str(values: &[Option<&Value>]) -> Option<String> {
	values.iter().find_map(|value| as_str(*value))
}

/// Image d'un item (poster / cover / recherche approfondie dans les clés).
fn image_of(item: &Value) -> Option<String> {
	const IMG_TERMS: [&str; 10] = [
		"image", "img", "thumb", "thumbnail", "poster", "cover", "icon", "banner", "pic",
		"picture",
	];

	let normalize = |raw: String| {
		if raw.starts_with("//") {
			Some(format!("https:{raw}"))
		} else if raw.starts_with("http") {
			Some(raw)
		} else {
			None
		}
	};

	let from_value = |value: Option<&Value>| -> Option<String> {
		match value? {
			Value::String(text) => normalize(text.trim().to_string()),
			Value::Object(map) => normalize(map.get("url")?.as_str()?.to_string()),
			_ => None,
		}
	};

	if let Some(found) = from_value(item.get("poster")).or_else(|| from_value(item.get("cover"))) {
		return Some(found);
	}

	for term in IMG_TERMS {
		if let Some(found) = from_value(item.get(term)) {
			return Some(found);
		}
	}

	if let Some(map) = item.as_object() {
		for (key, value) in map {
			let key_lower = key.to_lowercase();
			if IMG_TERMS.iter().any(|term| key_lower.contains(term)) {
				if let Some(found) = from_value(Some(value)) {
					return Some(found);
				}
			}
		}
	}

	from_value(item.get("horizontalPoster"))
		.or_else(|| from_value(item.get("horizontalCover")))
		.or_else(|| {
			let banner = item.get("banner")?;
			from_value(banner.get("image")).or_else(|| from_value(Some(banner)))
		})
}

fn genres_of(item: &Value) -> Vec<Value> {
	for key in ["genres", "genreNames", "genresNames", "genre"] {
		match item.get(key) {
			Some(Value::Array(items)) => {
				let names: Vec<Value> = items
					.iter()
					.filter_map(|entry| match entry {
						Value::String(text) => Some(Value::String(text.clone())),
						Value::Object(map) => map
							.get("name")
							.or_else(|| map.get("title"))
							.and_then(|value| value.as_str())
							.map(|value| Value::String(value.to_string())),
						_ => None,
					})
					.collect();
				if !names.is_empty() {
					return names;
				}
			}
			Some(Value::String(text)) => {
				return vec![Value::String(text.clone())];
			}
			_ => {}
		}
	}
	vec![]
}

fn score_of(item: &Value) -> Option<f64> {
	let raw = first_str(&[
		item.get("imdbRatingValue"),
		item.get("imdbRate"),
		item.get("starRating"),
		item.get("score"),
	])?;
	raw.parse::<f64>().ok().filter(|score| *score > 0.0)
}

fn year_of(item: &Value) -> Option<i64> {
	let raw = first_str(&[
		item.get("releaseTime"),
		item.get("releaseDate"),
		item.get("year"),
	])?;
	let slice = raw.get(..4)?;
	let year: i64 = slice.parse().ok()?;
	(year > 1800).then_some(year)
}

fn runtime_minutes(item: &Value) -> Option<i64> {
	let candidate = item
		.get("duration")
		.or_else(|| item.get("runtime"))
		.or_else(|| item.get("minute"))?;
	match candidate {
		Value::Number(value) => value.as_i64(),
		Value::String(text) => {
			let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();
			digits.parse::<i64>().ok()
		}
		_ => None,
	}
}

fn subject_type_of(item: &Value) -> i64 {
	if let Some(value) = item
		.get("subjectType")
		.or_else(|| item.get("type"))
		.or_else(|| item.get("subject_type"))
	{
		if let Some(number) = value.as_i64() {
			return number;
		}
		if let Some(text) = value.as_str() {
			if let Ok(number) = text.parse::<i64>() {
				return number;
			}
		}
	}
	if item.get("episodeCount").or(item.get("seasonCount")).is_some() {
		2
	} else {
		1
	}
}

/// Identifiant d'item (avec extraction depuis `deepLink` si nécessaire).
fn subject_id_of(item: &Value) -> Option<String> {
	let mut sid = first_str(&[
		item.get("subjectId"),
		item.get("subject_id"),
		item.get("id"),
		item.get("playlistId"),
	]);

	let deep_link = first_str(&[item.get("deepLink")]).unwrap_or_default();
	if deep_link.contains("subjectId=") && (sid.is_none() || sid.as_deref() == Some("0")) {
		sid = Some(
			deep_link
				.split("subjectId=")
				.nth(1)
				.unwrap_or("")
				.split('&')
				.next()
				.unwrap_or("")
				.to_string(),
		);
	} else if deep_link.contains("playlistId=") && (sid.is_none() || sid.as_deref() == Some("0")) {
		sid = Some(
			deep_link
				.split("playlistId=")
				.nth(1)
				.unwrap_or("")
				.split('&')
				.next()
				.unwrap_or("")
				.to_string(),
		);
	}

	sid.filter(|value| !value.is_empty() && value != "0")
}

/// Carte normalisée d'un item de l'API interne.
pub fn item_to_card(raw: &Value) -> Option<Value> {
	let item = match raw.get("subject") {
		Some(subject @ Value::Object(_)) => subject,
		_ => raw,
	};

	let sid = subject_id_of(item)?;
	let title = first_str(&[
		item.get("title"),
		item.get("name"),
		item.get("subjectName"),
		item.get("subject_name"),
		item.get("categoryName"),
		item.get("content"),
		item.get("keyword"),
		item.get("keywordName"),
		item.get("itemName"),
		item.get("show_name"),
		item.get("showTitle"),
		item.get("titleName"),
		item.get("title_en"),
		item.get("tag"),
		item.get("label"),
		item.get("subtitle"),
		item.get("tabName"),
		item.get("searchName"),
		item.get("promotionName"),
		raw.get("title"),
		raw.get("name"),
		raw.get("content"),
		raw.get("label"),
	])?;

	let image = image_of(item);
	let score = score_of(item);
	let year = year_of(item);
	let description = strip_html(
		item.get("description")
			.or_else(|| item.get("introduction"))
			.or_else(|| item.get("desc")),
	);

	Some(json!({
		"id": format!("{ID_PREFIX}{sid}"),
		"mediaType": "internal",
		"internalId": sid,
		"source": "internal",
		"sourceLabel": SOURCE_LABEL,
		"badge": "STREAMING",
		"title": title,
		"originalTitle": Value::Null,
		"overview": description,
		"poster": image.clone().map(Value::String).unwrap_or(Value::Null),
		"thumb": image.clone().map(Value::String).unwrap_or(Value::Null),
		"backdrop": image.map(Value::String).unwrap_or(Value::Null),
		"year": year.map(Value::from).unwrap_or(Value::Null),
		"releaseDate": Value::Null,
		"rating": score.map(round1).map(Value::from).unwrap_or(Value::Null),
		"votes": Value::Null,
		"popularity": 0,
		"genreIds": json!([]),
		"genres": Value::Array(genres_of(item)),
		"runtime": runtime_minutes(item).map(Value::from).unwrap_or(Value::Null),
		"seasons": item.get("seasonCount").cloned().unwrap_or(Value::Null),
		"episodes": item.get("episodeCount").cloned().unwrap_or(Value::Null),
		"originalLanguage": item
			.get("lanCode")
			.or_else(|| item.get("language"))
			.cloned()
			.unwrap_or(Value::Null),
		"subjectType": subject_type_of(item),
	}))
}

fn collection_path(collection: &str) -> String {
	let known = COLLECTIONS.iter().any(|(id, _)| *id == collection);
	if known && collection != DEFAULT_COLLECTION {
		format!("/{collection}")
	} else if collection == DEFAULT_COLLECTION {
		"/home".to_string()
	} else {
		"/home".to_string()
	}
}

fn cards_of(payload: &Value) -> Vec<Value> {
	extract_items(payload)
		.iter()
		.filter_map(item_to_card)
		.collect()
}

/// Recherche interne (`/search`) ou rubrique (`/home`, `/movies`… sans terme).
pub async fn search(q: &str, collection: &str, page: i64, rows: i64) -> Result<Value> {
	let safe_page = page.max(1);
	let query: Vec<(&str, String)> = vec![("page", safe_page.to_string())];

	let payload = if q.trim().is_empty() {
		internal_fetch(&collection_path(collection), &query, config().ttl.short).await?
	} else {
		let search_query: Vec<(&str, String)> = vec![
			("q", q.trim().to_string()),
			("page", safe_page.to_string()),
		];
		internal_fetch("/search", &search_query, config().ttl.short).await?
	};

	let mut items = cards_of(&payload);
	items.truncate(rows as usize);

	Ok(json!({
		"page": safe_page,
		"rows": rows,
		"license": "free",
		"total": Value::Null,
		"hasMore": items.len() >= rows as usize,
		"items": items,
	}))
}

/// Cartes de la page d'accueil de l'API interne (rail de l'accueil Movie Box).
pub async fn home_cards() -> Result<Vec<Value>> {
	let payload = internal_fetch("/home", &[], config().ttl.short).await?;
	Ok(cards_of(&payload).into_iter().take(20).collect())
}

fn extension_of(url: &str) -> &'static str {
	let lower = url.to_lowercase();
	if lower.contains(".m3u8") {
		"HLS"
	} else if lower.contains(".mpd") {
		"DASH"
	} else if lower.contains(".mp4") || lower.contains(".m4v") {
		"MP4"
	} else if lower.contains(".webm") {
		"WebM"
	} else {
		"Vidéo"
	}
}

/// Priorité navigateur : MP4 > HLS > DASH, et les flux H.265 (incompatibles HTML5) en dernier.
fn stream_priority(url: &str) -> u8 {
	let lower = url.to_lowercase();
	if lower.contains("h265") || lower.contains("x265") || lower.contains("hev1") {
		return 10;
	}
	if lower.ends_with(".mp4") || lower.contains(".mp4?") {
		return 0;
	}
	if lower.contains(".m3u8") {
		return 1;
	}
	5
}

/// URL de lecture passée par le proxy local (évite les contraintes CORS côté navigateur).
fn proxy_url(src: &str, cookie: &str) -> String {
	format!(
		"/api/internal/proxy?src={}&cookie={}",
		urlencode(src),
		urlencode(cookie)
	)
}

/// Résout les flux de lecture d'un titre interne (relus via le proxy local).
pub async fn streams(
	id: &str,
	season: i64,
	episode: i64,
	quality: &str,
) -> Result<Vec<Value>> {
	let query: Vec<(&str, String)> = vec![
		("season", season.to_string()),
		("episode", episode.to_string()),
		("quality", quality.to_string()),
	];
	// Les URL signées expirent : cache très court.
	let payload = internal_fetch(&format!("/stream/{id}"), &query, 60_000).await?;

	let data = payload.get("data").cloned().unwrap_or(json!({}));
	let global_cookie = first_str(&[
		payload.get("signCookie"),
		data.get("signCookie"),
	])
	.unwrap_or_default();

	let raw_streams = data
		.get("streamList")
		.or_else(|| data.get("streams"))
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();

	let mut mapped: Vec<(u8, Value)> = raw_streams
		.iter()
		.filter_map(|stream| {
			let url = stream.get("url").and_then(|value| value.as_str())?;
			if url.is_empty() {
				return None;
			}
			let cookie = stream
				.get("signCookie")
				.and_then(|value| value.as_str())
				.filter(|value| !value.is_empty())
				.unwrap_or(&global_cookie)
				.to_string();
			let quality_label = first_str(&[stream.get("quality")])
				.unwrap_or_else(|| "Auto".to_string());
			let codec = first_str(&[stream.get("codec")]).unwrap_or_default();
			let format = extension_of(url);

			Some((
				stream_priority(url),
				json!({
					"name": quality_label,
					"format": if codec.is_empty() { format.to_string() } else { format!("{format} · {codec}") },
					"quality": quality_label,
					"size": stream.get("size").and_then(|value| value.as_u64()).unwrap_or(0),
					"url": proxy_url(url, &cookie),
					"directUrl": url,
					"duration": stream.get("duration").cloned().unwrap_or(Value::Null),
				}),
			))
		})
		.collect();

	mapped.sort_by_key(|(priority, _)| *priority);
	Ok(mapped.into_iter().map(|(_, stream)| stream).collect())
}

/// Résout l'URL distante (CDN, signée) d'un flux pour le téléchargement.
/// `file_name` sert à sélectionner la qualité voulue.
pub async fn resolve_download(
	id: &str,
	season: i64,
	episode: i64,
	file_name: &str,
) -> Result<Value> {
	let query: Vec<(&str, String)> = vec![
		("season", season.max(1).to_string()),
		("episode", episode.max(1).to_string()),
	];
	// Les URL signées expirent : ne jamais réutiliser le cache pour un lien CDN.
	let payload = internal_fetch(&format!("/stream/{id}"), &query, 0).await?;
	let data = payload.get("data").cloned().unwrap_or(json!({}));
	let raw_streams = data
		.get("streamList")
		.or_else(|| data.get("streams"))
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();

	let wanted = parse_quality(file_name);
	let chosen = wanted
		.as_deref()
		.and_then(|quality| {
			// Qualité exacte d'abord.
			raw_streams.iter().find(|stream| {
				stream.get("quality").and_then(|value| value.as_str()) == Some(quality)
			})
		})
		// Sinon la plus proche (360/480/720/1080) : un épisode qui n'existe pas
		// dans la qualité demandée ne doit pas retomber sur la plus basse.
		.or_else(|| {
			let wanted_height = wanted.as_deref().and_then(quality_height);
			raw_streams
				.iter()
				.filter_map(|stream| {
					let label = stream.get("quality").and_then(|value| value.as_str())?;
					let height = quality_height(label)?;
					Some((wanted_height.map_or(i64::MAX, |target| (height - target).abs()), stream))
				})
				.min_by_key(|(distance, _)| *distance)
				.map(|(_, stream)| stream)
		})
		.or_else(|| raw_streams.first())
		.ok_or_else(|| ApiError::not_found("Aucun flux téléchargeable pour ce titre"))?;

	let url = chosen
		.get("url")
		.and_then(|value| value.as_str())
		.filter(|value| !value.is_empty())
		.ok_or_else(|| ApiError::not_found("Flux introuvable pour ce titre"))?;

	Ok(json!({
		"identifier": format!("{ID_PREFIX}{id}"),
		"name": file_name,
		"size": chosen.get("size").and_then(|value| value.as_u64()).unwrap_or(0),
		"format": extension_of(url),
		"url": url,
	}))
}

/// Extrait une qualité du type `720p` / `1080p` depuis un nom de fichier.
/// Hauteur vidéo d'un libellé de qualité (« 1080p » -> 1080).
fn quality_height(label: &str) -> Option<i64> {
	let digits: String = label.chars().take_while(|c| c.is_ascii_digit()).collect();
	digits.parse::<i64>().ok().filter(|height| *height >= 144)
}

fn parse_quality(file_name: &str) -> Option<String> {
	let lowered = file_name.to_lowercase();
	let bytes = lowered.as_bytes();
	let mut index = 0;
	while index < bytes.len() {
		if bytes[index].is_ascii_digit() {
			let start = index;
			while index < bytes.len() && bytes[index].is_ascii_digit() {
				index += 1;
			}
			if index < bytes.len() && bytes[index] == b'p' {
				let digits = &lowered[start..index];
				if (3..=4).contains(&digits.len()) {
					return Some(format!("{digits}p"));
				}
			}
		} else {
			index += 1;
		}
	}
	None
}

/// Nettoie une chaîne pour servir de nom de fichier (sans extension).
fn sanitize_file_stem(input: &str) -> String {
	let cleaned: String = input
		.chars()
		.map(|c| match c {
			'/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
			_ => c,
		})
		.collect();
	let trimmed = cleaned.trim();
	if trimmed.is_empty() {
		"video".to_string()
	} else {
		trimmed.chars().take(120).collect()
	}
}

/// Épisodes d'une série interne (`/episodes/{id}`), aplatis au format Movie Box.
pub async fn episodes(id: &str) -> Result<Vec<Value>> {
	let payload = match internal_fetch(&format!("/episodes/{id}"), &[], 60_000).await {
		Ok(payload) => payload,
		Err(_) => return Ok(vec![]),
	};

	let data = payload.get("data").cloned().unwrap_or(json!({}));
	let seasons = data
		.get("seasons")
		.or_else(|| data.get("seasonList"))
		.or_else(|| data.get("list"))
		.or_else(|| payload.get("seasons"))
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();

	let mut flattened: Vec<Value> = Vec::new();

	for season in &seasons {
		let season_number = season
			.get("se")
			.or_else(|| season.get("seasonNumber"))
			.and_then(|value| value.as_i64())
			.unwrap_or(1);

		let mut numbers: Vec<(i64, Option<String>)> = Vec::new();
		for key in ["allEp", "epList", "episodeList", "episodes", "list", "items"] {
			match season.get(key) {
				Some(Value::String(pool)) => {
					numbers.extend(
						pool.split(',')
							.filter_map(|raw| raw.trim().parse::<i64>().ok())
							.map(|number| (number, None)),
					);
				}
				Some(Value::Array(pool)) => {
					for item in pool {
						match item {
							Value::Number(value) => {
								if let Some(number) = value.as_i64() {
									numbers.push((number, None));
								}
							}
							Value::String(text) => {
								if let Ok(number) = text.parse::<i64>() {
									numbers.push((number, None));
								}
							}
							Value::Object(map) => {
								if let Some(number) = map
									.get("ep")
									.or_else(|| map.get("episodeNumber"))
									.or_else(|| map.get("episode_number"))
									.and_then(|value| value.as_i64())
								{
									numbers.push((number, first_str(&[map.get("title")])));
								}
							}
							_ => {}
						}
					}
				}
				_ => {}
			}
			if !numbers.is_empty() {
				break;
			}
		}

		if numbers.is_empty() {
			if let Some(max_ep) = season
				.get("maxEp")
				.or_else(|| season.get("max_ep"))
				.and_then(|value| value.as_i64())
			{
				numbers.extend((1..=max_ep).map(|number| (number, None)));
			}
		}

		for (number, title) in numbers {
			flattened.push(json!({
				"season": season_number,
				"number": number,
				"label": super::archive::episode_label(Some(season_number), number),
				"title": title.unwrap_or_else(|| format!("Épisode {number}")),
				"streams": json!([]),
			}));
		}
	}

	// Tri naturel saison -> numéro : l'interface et l'assistant de téléchargement
	// supposent une liste ordonnée.
	flattened.sort_by_key(|episode| {
		let season = episode
			.get("season")
			.and_then(|value| value.as_i64())
			.unwrap_or(1);
		let number = episode
			.get("number")
			.and_then(|value| value.as_i64())
			.unwrap_or(0);
		(season, number)
	});

	// Garde-fou large : certaines séries dépassent 300 épisodes (8 saisons et plus).
	// Une coupure trop basse faisait disparaître les dernières saisons.
	flattened.truncate(1000);
	Ok(flattened)
}

/// Fiche complète d'un titre interne : détail + flux (via proxy) + épisodes.
pub async fn title(id: &str) -> Result<Value> {
	let id = id.trim();
	if id.is_empty() {
		return Err(ApiError::not_found("Titre introuvable"));
	}

	let payload = internal_fetch(&format!("/detail/{id}"), &[], config().ttl.medium).await?;
	let data = payload.get("data").cloned().unwrap_or(json!({}));
	if !data.is_object() || data.as_object().map(|map| map.is_empty()).unwrap_or(true) {
		return Err(ApiError::not_found(
			"Ce titre n'existe pas dans l'API interne",
		));
	}

	let card = item_to_card(&data)
		.ok_or_else(|| ApiError::not_found("Ce titre n'existe pas dans l'API interne"))?;

	let subject_type = card
		.get("subjectType")
		.and_then(|value| value.as_i64())
		.unwrap_or(1);
	let is_movie = subject_type == 1;

	// Épisodes (séries uniquement) + flux de l'épisode courant.
	let mut episode_list = if is_movie {
		vec![]
	} else {
		episodes(id).await.unwrap_or_default()
	};
	let (first_season, first_episode) = episode_list
		.first()
		.map(|episode| {
			(
				episode
					.get("season")
					.and_then(|value| value.as_i64())
					.unwrap_or(1),
				episode
					.get("number")
					.and_then(|value| value.as_i64())
					.unwrap_or(1),
			)
		})
		.unwrap_or((1, 1));

	let streams = streams(id, first_season, first_episode, "720p")
		.await
		.unwrap_or_default();

	// Nom de fichier proposé pour le téléchargement (qualité + extension),
	// construit à partir du titre afin de produire un fichier lisible.
	let title_stem = card
		.get("title")
		.and_then(|value| value.as_str())
		.unwrap_or("video")
		.to_string();
	let file_stem = sanitize_file_stem(&title_stem);
	let streams: Vec<Value> = streams
		.into_iter()
		.map(|mut stream| {
			if let Some(object) = stream.as_object_mut() {
				let quality = object
					.get("quality")
					.and_then(|value| value.as_str())
					.unwrap_or("video")
					.to_string();
				let ext = object
					.get("directUrl")
					.and_then(|value| value.as_str())
					.map(extension_of)
					.unwrap_or("MP4")
					.to_lowercase();
				object.insert(
					"fileName".to_string(),
					Value::String(format!("{file_stem} {quality}.{ext}")),
				);
			}
			stream
		})
		.collect();

	if let Some(first) = episode_list.first_mut() {
		if let Some(object) = first.as_object_mut() {
			object.insert("streams".to_string(), Value::Array(streams.clone()));
		}
	}

	// Cast et éléments liés (collections / recommandations).
	let cast_source = data
		.get("cast")
		.or_else(|| data.get("staffList"))
		.or_else(|| data.get("actorList"))
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();
	let cast: Vec<Value> = cast_source
		.iter()
		.take(16)
		.map(|member| {
			json!({
				"id": member.get("id").cloned().unwrap_or(Value::Null),
				"name": first_str(&[member.get("name"), member.get("title")]).map(Value::String).unwrap_or(Value::Null),
				"character": Value::Null,
				"photo": image_of(member).map(Value::String).unwrap_or(Value::Null),
			})
		})
		.collect();
	let credits = json!({ "cast": cast, "directors": [], "writers": [] });

	let related_source = data
		.get("collectionItems")
		.or_else(|| data.get("items"))
		.or_else(|| data.get("relatedSubjects"))
		.or_else(|| data.get("recommendSubjects"))
		.cloned()
		.unwrap_or(Value::Array(vec![]));
	let related_items = extract_items(&json!({ "list": related_source }));
	let similar: Vec<Value> = related_items
		.iter()
		.filter_map(item_to_card)
		.filter(|card| card.get("internalId").and_then(|value| value.as_str()) != Some(id))
		.take(18)
		.collect();

	let seasons: Vec<Value> = {
		let mut by_season: std::collections::BTreeMap<i64, i64> =
			std::collections::BTreeMap::new();
		for episode in &episode_list {
			let season = episode
				.get("season")
				.and_then(|value| value.as_i64())
				.unwrap_or(1);
			*by_season.entry(season).or_insert(0) += 1;
		}
		by_season
			.iter()
			.map(|(season, count)| {
				json!({
					"seasonNumber": season,
					"name": format!("Saison {season}"),
					"episodeCount": count,
					"airDate": Value::Null,
					"poster": card.get("poster").cloned().unwrap_or(Value::Null),
					"overview": "",
				})
			})
			.collect()
	};

	let title_text = card
		.get("title")
		.and_then(|value| value.as_str())
		.unwrap_or("")
		.to_string();
	let year = card.get("year").and_then(|value| value.as_i64());

	let mut result = card.as_object().cloned().unwrap_or_default();
	let playable = !streams.is_empty();
	result.insert("playable".to_string(), Value::Bool(playable));
	result.insert("player".to_string(), Value::String("html5".to_string()));
	// Identifiant exploité par le gestionnaire de téléchargements.
	result.insert(
		"sourceId".to_string(),
		Value::String(format!("{ID_PREFIX}{id}")),
	);
	result.insert("tagline".to_string(), Value::Null);
	result.insert("status".to_string(), Value::Null);
	result.insert("homepage".to_string(), Value::Null);
	result.insert("networks".to_string(), json!([]));
	result.insert("studios".to_string(), json!([]));
	result.insert("countries".to_string(), json!([]));
	result.insert("spokenLanguages".to_string(), json!([]));
	result.insert("budget".to_string(), Value::Null);
	result.insert("revenue".to_string(), Value::Null);
	result.insert("ageRating".to_string(), Value::Null);
	result.insert("credits".to_string(), credits);
	result.insert("trailer".to_string(), Value::Null);
	result.insert(
		"providers".to_string(),
		json!({ "region": config().watch_region, "link": null, "groups": [] }),
	);
	result.insert(
		"legalLinks".to_string(),
		build_legal_links(&title_text, year, "movie", None),
	);
	result.insert("screenshots".to_string(), json!([]));
	result.insert("seasons".to_string(), Value::Array(seasons));
	result.insert("similar".to_string(), Value::Array(similar));
	result.insert("imdbId".to_string(), Value::Null);
	result.insert("legal".to_string(), super::providers::legal_info());
	result.insert("streams".to_string(), Value::Array(streams));
	result.insert("episodes".to_string(), Value::Array(episode_list));
	result.insert(
		"kind".to_string(),
		Value::String(if is_movie { "movie" } else { "series" }.to_string()),
	);
	result.insert("subjects".to_string(), json!([]));
	result.insert("embedUrl".to_string(), Value::Null);
	result.insert("detailsUrl".to_string(), Value::Null);
	result.insert(
		"sourceLabel".to_string(),
		Value::String(SOURCE_LABEL.to_string()),
	);
	result.insert(
		"externalLinks".to_string(),
		json!([{
			"id": "internal",
			"name": "MovieBox (API interne)",
			"description": "Source du flux : moviebox-internal-api",
			"url": format!("{}/detail/{}", config().internal_api_url, urlencode(id)),
			"free": true,
		}]),
	);
	result.insert(
		"streamNotice".to_string(),
		Value::String(
			"Flux résolus par l'API interne moviebox-internal-api et relus via le proxy local."
				.to_string(),
		),
	);

	Ok(Value::Object(result))
}

/// État de la source interne (utilisé par `/api/health` et `/api/internal/health`).
pub async fn health() -> Value {
	let base = config().internal_api_url.clone();
	if !enabled() {
		return json!({ "enabled": false, "reachable": false, "baseUrl": base });
	}

	match internal_fetch("/health", &[], 0).await {
		Ok(payload) => json!({
			"enabled": true,
			"reachable": true,
			"baseUrl": base,
			"upstream": payload
		}),
		Err(_) => json!({ "enabled": true, "reachable": false, "baseUrl": base }),
	}
}
