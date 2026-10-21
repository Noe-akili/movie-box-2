//! Source MovieBox native (remplace le service Python `source_server.py` +
//! `moviebox_api`). Reproduit fidèlement le contrat JSON consommé par
//! `services::internal` :
//!
//! - `/home` `/trending` `/movies` … : rubriques (tendances ou recherche)
//! - `/search?q=&page=` : recherche
//! - `/detail/{id}` : fiche
//! - `/episodes/{id}` : saisons
//! - `/stream/{id}?season=&episode=&quality=` : flux de lecture
//!
//! Toute la logique est portée depuis le paquet Python d'origine :
//! session cookies, résolution d'URL absolue, extraction du payload Nuxt
//! (`__NUXT_DATA__`), et lecture des fichiers téléchargeables.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use crate::config::config;
use crate::error::{ApiError, Result};
use crate::http::build_url;

const WEB_HOST_DEFAULT: &str = "h5.aoneroom.com";
const API_HOST_DEFAULT: &str = "h5-api.aoneroom.com";
const OLD_API_PREFIX: &str = "/wefeed-h5-bff/";
const API_PREFIX: &str = "/wefeed-h5api-bff/";

const USER_AGENT: &str =
	"Mozilla/5.0 (X11; Linux x86_64; rv:137.0) Gecko/20100101 Firefox/137.0";
const ACCEPT_LANGUAGE: &str = "en-US,en;q=0.5";
const REQUEST_TIMEOUT_MS: u64 = 30_000;
const COOKIE_TTL_SECS: u64 = 600;
const SEARCH_PER_PAGE: i64 = 24;

/// `MOVIEBOX_API_HOST` permet de changer de miroir (comme le paquet Python).
fn override_host() -> Option<String> {
	std::env::var("MOVIEBOX_API_HOST")
		.ok()
		.map(|value| value.trim().to_string())
		.filter(|value| !value.is_empty())
		.map(|value| {
			value
				.trim_start_matches("https://")
				.trim_start_matches("http://")
				.trim_end_matches('/')
				.to_string()
		})
}

fn web_host() -> &'static str {
	static HOST: LazyLock<String> = LazyLock::new(|| {
		override_host().unwrap_or_else(|| WEB_HOST_DEFAULT.to_string())
	});
	&HOST
}

fn api_host() -> &'static str {
	static HOST: LazyLock<String> = LazyLock::new(|| {
		override_host().unwrap_or_else(|| API_HOST_DEFAULT.to_string())
	});
	&HOST
}

/// Base web publique (`https://h5.aoneroom.com/`), utilisée pour les liens externes.
pub fn web_base() -> String {
	format!("https://{}/", web_host())
}

fn strip_scheme_host(url: &str) -> String {
	for prefix in ["http://", "https://"] {
		if let Some(rest) = url.strip_prefix(prefix) {
			return match rest.find('/') {
				Some(index) => rest[index..].to_string(),
				None => "/".to_string(),
			};
		}
	}
	url.to_string()
}

/// Reproduit `moviebox_api.helpers.get_absolute_url`.
pub fn absolute_url(relative: &str) -> String {
	let path = strip_scheme_host(relative);

	if let Some(rest) = path.strip_prefix(OLD_API_PREFIX) {
		let rel = rest.strip_prefix("web/").unwrap_or(rest);
		if rel.starts_with("subject/download") || rel.starts_with("subject/play") {
			format!("https://{}{}", web_host(), path)
		} else {
			format!("https://{}{}{}", api_host(), API_PREFIX, rel)
		}
	} else {
		format!("https://{}{}", web_host(), path)
	}
}

// --- Client HTTP + session cookies ---------------------------------------

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
	let mut headers = reqwest::header::HeaderMap::new();
	headers.insert(
		reqwest::header::ACCEPT_LANGUAGE,
		ACCEPT_LANGUAGE.parse().unwrap(),
	);
	headers.insert(
		reqwest::header::REFERER,
		web_base().parse().expect("referer"),
	);

	reqwest::Client::builder()
		.user_agent(USER_AGENT)
		.default_headers(headers)
		.cookie_store(true)
		.connect_timeout(Duration::from_secs(10))
		.timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
		.build()
		.expect("client MovieBox")
});

static COOKIE_REFRESH: LazyLock<tokio::sync::Mutex<Option<Instant>>> =
	LazyLock::new(|| tokio::sync::Mutex::new(None));

fn reqwest_error(error: reqwest::Error) -> ApiError {
	if error.is_timeout() {
		ApiError::new(504, "Délai dépassé lors de l'appel à MovieBox")
	} else if error.is_connect() {
		ApiError::new(504, "Connexion à MovieBox impossible")
	} else {
		ApiError::new(502, format!("Erreur réseau MovieBox : {error}"))
	}
}

/// Récupère (et met en cache) le cookie `token` requis par l'API MovieBox.
async fn ensure_cookies(force: bool) -> Result<()> {
	let mut guard = COOKIE_REFRESH.lock().await;
	if !force {
		if let Some(fetched_at) = *guard {
			if fetched_at.elapsed() < Duration::from_secs(COOKIE_TTL_SECS) {
				return Ok(());
			}
		}
	}

	let url = absolute_url("/wefeed-h5-bff/app/get-latest-app-pkgs?app_name=moviebox");
	let response = CLIENT
		.get(&url)
		.timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
		.send()
		.await
		.map_err(reqwest_error)?;

	// Le simple fait de recevoir la réponse dépose le cookie dans le jar.
	let _ = response.bytes().await;
	*guard = Some(Instant::now());
	Ok(())
}

enum Verb {
	Get,
	Post,
}

async fn request(
	verb: Verb,
	url: &str,
	accept: &str,
	referer: Option<&str>,
	body: Option<&Value>,
) -> Result<reqwest::Response> {
	let method = match verb {
		Verb::Get => reqwest::Method::GET,
		Verb::Post => reqwest::Method::POST,
	};

	let mut refreshed = false;
	loop {
		ensure_cookies(refreshed).await?;

		let mut req = CLIENT
			.request(method.clone(), url)
			.header(reqwest::header::ACCEPT, accept);
		if let Some(referer) = referer {
			req = req.header(reqwest::header::REFERER, referer);
		}
		if let Some(body) = body {
			req = req.json(body);
		}

		let response = req.send().await.map_err(reqwest_error)?;
		let status = response.status();

		if (status == reqwest::StatusCode::UNAUTHORIZED
			|| status == reqwest::StatusCode::FORBIDDEN)
			&& !refreshed
		{
			refreshed = true;
			continue;
		}

		if !status.is_success() {
			return Err(ApiError::upstream(
				502,
				format!("Service MovieBox indisponible ({})", status.as_u16()),
			));
		}

		return Ok(response);
	}
}

async fn get_json(url: &str, referer: Option<&str>) -> Result<Value> {
	let response = request(Verb::Get, url, "application/json", referer, None).await?;
	let text = response.text().await.map_err(reqwest_error)?;
	serde_json::from_str(&text)
		.map_err(|_| ApiError::upstream(502, "Réponse MovieBox illisible (JSON invalide)"))
}

async fn post_json(url: &str, body: &Value) -> Result<Value> {
	let response = request(Verb::Post, url, "application/json", None, Some(body)).await?;
	let text = response.text().await.map_err(reqwest_error)?;
	serde_json::from_str(&text)
		.map_err(|_| ApiError::upstream(502, "Réponse MovieBox illisible (JSON invalide)"))
}

async fn get_text(url: &str) -> Result<String> {
	let response = request(
		Verb::Get,
		url,
		"text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
		None,
		None,
	)
	.await?;
	response.text().await.map_err(reqwest_error)
}

/// Reproduit `process_api_response` : succès si `code == 0` et `message == "ok"`.
fn process_api(value: Value) -> Result<Value> {
	let code_ok = value
		.get("code")
		.and_then(Value::as_i64)
		.is_some_and(|code| code == 0);
	let message_ok = value.get("message").and_then(Value::as_str) == Some("ok");

	if code_ok && message_ok {
		Ok(value.get("data").cloned().unwrap_or(Value::Null))
	} else {
		let message = value
			.get("message")
			.and_then(Value::as_str)
			.unwrap_or("réponse inattendue");
		Err(ApiError::upstream(
			502,
			format!("Réponse MovieBox invalide : {message}"),
		))
	}
}

// --- Extraction de la fiche (payload Nuxt `__NUXT_DATA__`) ----------------

fn extract_json_script(html: &str) -> Option<&str> {
	let mut cursor = 0;
	while let Some(offset) = html[cursor..].find("<script") {
		let start = cursor + offset;
		let tag_end = start + html[start..].find('>')? + 1;
		let tag = &html[start..tag_end];
		if tag.contains("application/json") {
			let close = html[tag_end..].find("</script>")?;
			return Some(&html[tag_end..tag_end + close]);
		}
		cursor = tag_end;
	}
	None
}

/// Résout récursivement une valeur d'un payload « devalue »/Nuxt.
fn resolve_value(value: &Value, root: &[Value]) -> Value {
	match value {
		Value::Array(items) => Value::Array(
			items
				.iter()
				.map(|item| match item {
					Value::Number(number) => number
						.as_u64()
						.and_then(|index| root.get(index as usize))
						.map(|value| resolve_value(value, root))
						.unwrap_or(Value::Null),
					other => resolve_value(other, root),
				})
				.collect(),
		),
		Value::Object(map) => {
			let mut resolved = Map::new();
			for (key, value) in map {
				let entry = value
					.as_u64()
					.and_then(|index| root.get(index as usize))
					.map(|value| resolve_value(value, root))
					.unwrap_or(Value::Null);
				resolved.insert(key.clone(), entry);
			}
			Value::Object(resolved)
		}
		other => other.clone(),
	}
}

/// Extrait `extracts[0]["state"][1]` du script JSON puis retire le préfixe
/// `$s` (2 caractères) de chaque clé, comme `JsonDetailsExtractor`.
pub fn extract_res_data(html: &str) -> Result<Value> {
	let raw = extract_json_script(html).ok_or_else(|| {
		ApiError::upstream(502, "Réponse MovieBox invalide (données de fiche absentes)")
	})?;

	let root: Vec<Value> = serde_json::from_str(raw)
		.map_err(|_| ApiError::upstream(502, "Données de fiche MovieBox illisibles"))?;
	if root.is_empty() {
		return Err(ApiError::upstream(502, "Données de fiche MovieBox vides"));
	}

	let mut extracts: Vec<Map<String, Value>> = Vec::new();
	for entry in &root {
		if let Value::Object(map) = entry {
			let mut details = Map::new();
			for (key, index) in map {
				let value = index
					.as_u64()
					.and_then(|position| root.get(position as usize))
					.cloned()
					.unwrap_or(Value::Null);
				details.insert(key.clone(), resolve_value(&value, &root));
			}
			extracts.push(details);
		}
	}

	let first = extracts
		.into_iter()
		.next()
		.ok_or_else(|| ApiError::upstream(502, "Structure de fiche MovieBox inattendue"))?;

	let target = first
		.get("state")
		.and_then(Value::as_array)
		.and_then(|state| state.get(1))
		.and_then(Value::as_object)
		.ok_or_else(|| ApiError::upstream(502, "Structure de fiche MovieBox inattendue"))?;

	let mut out = Map::new();
	for (key, value) in target {
		if key.len() < 2 {
			continue;
		}
		out.insert(key[2..].to_string(), value.clone());
	}
	Ok(Value::Object(out))
}

// --- Registre subjectId -> item (reconstruit les URLs) --------------------

static REGISTRY: LazyLock<Mutex<HashMap<String, Value>>> = LazyLock::new(|| {
	Mutex::new(load_registry())
});

fn registry_file() -> std::path::PathBuf {
	config().downloads_dir.join("source_items.json")
}

fn load_registry() -> HashMap<String, Value> {
	let Ok(content) = std::fs::read_to_string(registry_file()) else {
		return HashMap::new();
	};
	serde_json::from_str::<HashMap<String, Value>>(&content).unwrap_or_default()
}

fn persist_registry(items: &HashMap<String, Value>) {
	let path = registry_file();
	if let Some(parent) = path.parent() {
		let _ = std::fs::create_dir_all(parent);
	}
	if let Ok(serialized) = serde_json::to_string(items) {
		let _ = std::fs::write(path, serialized);
	}
}

fn remember(items: &[Value]) {
	let mut registry = REGISTRY.lock().unwrap();
	for item in items {
		let Some(id) = item
			.get("subjectId")
			.and_then(Value::as_str)
			.map(str::to_string)
			.or_else(|| {
				item.get("subjectId")
					.and_then(Value::as_i64)
					.map(|id| id.to_string())
			})
		else {
			continue;
		};
		registry.insert(id, item.clone());
	}
	if registry.len() > 1000 {
		let keep: Vec<String> = registry.keys().take(1000).cloned().collect();
		registry.retain(|key, _| keep.iter().any(|kept| kept == key));
	}
	persist_registry(&registry);
}

fn recall(id: &str) -> Option<Value> {
	REGISTRY.lock().unwrap().get(id).cloned()
}

// --- Normalisation d'un item ----------------------------------------------

fn string_of(value: Option<&Value>) -> Option<String> {
	match value? {
		Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
		Value::Number(number) => Some(number.to_string()),
		_ => None,
	}
}

fn split_csv(value: Option<&Value>) -> Vec<Value> {
	match value {
		Some(Value::Array(items)) => items.clone(),
		Some(Value::String(text)) => text
			.split(',')
			.map(|part| part.trim())
			.filter(|part| !part.is_empty())
			.map(|part| Value::String(part.to_string()))
			.collect(),
		_ => vec![],
	}
}

fn trailer_url(item: &Value) -> Option<String> {
	match item.get("trailer")? {
		Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
		Value::Object(map) => map
			.get("videoAddress")
			.and_then(|address| address.get("url"))
			.and_then(Value::as_str)
			.map(str::to_string),
		_ => None,
	}
}

/// Reproduit `source_server.as_card` : sérialise un item MovieBox.
fn as_card(item: &Value) -> Option<Value> {
	let subject_id = string_of(item.get("subjectId"))?;
	let cover = item
		.get("cover")
		.and_then(|cover| cover.get("url"))
		.and_then(Value::as_str)
		.map(|url| json!({ "url": url }));

	Some(json!({
		"subjectId": subject_id,
		"subjectType": item.get("subjectType").and_then(Value::as_i64).unwrap_or(0),
		"title": item.get("title").and_then(Value::as_str).unwrap_or(""),
		"description": item.get("description").and_then(Value::as_str).unwrap_or(""),
		"releaseDate": item.get("releaseDate").cloned().unwrap_or(Value::Null),
		"duration": item.get("duration").cloned().unwrap_or(Value::Null),
		"genre": split_csv(item.get("genre")),
		"cover": cover,
		"countryName": item.get("countryName").and_then(Value::as_str).unwrap_or(""),
		"imdbRatingValue": item.get("imdbRatingValue").cloned().unwrap_or(Value::Null),
		"trailer": trailer_url(item).map(Value::String).unwrap_or(Value::Null),
		"detailPath": item.get("detailPath").and_then(Value::as_str).unwrap_or(""),
		"hasResource": item.get("hasResource").and_then(Value::as_bool).unwrap_or(false),
	}))
}

// --- Opérations amont ------------------------------------------------------

async fn fetch_trending(page: i64, per_page: i64) -> Result<Vec<Value>> {
	let url = build_url(
		&absolute_url("/wefeed-h5-bff/web/subject/trending"),
		&[
			("page", page.max(0).to_string()),
			("perPage", per_page.to_string()),
		],
	);
	let data = process_api(get_json(&url, None).await?)?;
	Ok(data
		.get("subjectList")
		.and_then(Value::as_array)
		.cloned()
		.unwrap_or_default())
}

async fn fetch_search(query: &str, page: i64, per_page: i64) -> Result<Vec<Value>> {
	let url = absolute_url("/wefeed-h5-bff/web/subject/search");
	let body = json!({
		"keyword": query,
		"page": page.max(1),
		"perPage": per_page,
		"subjectType": 0,
	});
	let data = process_api(post_json(&url, &body).await?)?;
	Ok(data
		.get("items")
		.and_then(Value::as_array)
		.cloned()
		.unwrap_or_default())
}

fn collection_query(collection: &str) -> Option<&'static str> {
	match collection {
		"movies" => Some("action"),
		"anime" => Some("anime"),
		"nollywood" => Some("nollywood"),
		"asian" => Some("asian"),
		"western" => Some("hollywood"),
		"kids" => Some("kids"),
		"music" => Some("music"),
		"education" => Some("documentary"),
		// home / trending / inconnu : rail de tendances.
		_ => None,
	}
}

async fn fetch_res_data(id: &str) -> Result<Value> {
	let item = recall(id)
		.ok_or_else(|| ApiError::not_found("Ce titre n'existe pas dans la source MovieBox"))?;
	let detail_path = item
		.get("detailPath")
		.and_then(Value::as_str)
		.filter(|path| !path.trim().is_empty())
		.ok_or_else(|| ApiError::not_found("Fiche MovieBox introuvable"))?;

	let page_url = format!("/detail/{}?id={}", detail_path, id);
	let html = get_text(&absolute_url(&page_url)).await?;
	extract_res_data(&html)
}

fn lenient_u64(value: Option<&Value>) -> u64 {
	match value {
		Some(Value::Number(number)) => number.as_u64().unwrap_or(0),
		Some(Value::String(text)) => text.trim().parse::<u64>().unwrap_or(0),
		_ => 0,
	}
}

async fn fetch_streams(id: &str, season: i64, episode: i64) -> Result<Vec<Value>> {
	let item = recall(id)
		.ok_or_else(|| ApiError::not_found("Ce titre n'existe pas dans la source MovieBox"))?;
	let detail_path = item
		.get("detailPath")
		.and_then(Value::as_str)
		.filter(|path| !path.trim().is_empty())
		.ok_or_else(|| ApiError::not_found("Fiche MovieBox introuvable"))?;

	// Les films utilisent toujours se=0/ep=0 (voir `DownloadableMovieFilesDetail`).
	let is_movie = item.get("subjectType").and_then(Value::as_i64) == Some(1);
	let (season, episode) = if is_movie {
		(0, 0)
	} else {
		(season, episode)
	};

	let url = build_url(
		&absolute_url("/wefeed-h5-bff/web/subject/download"),
		&[
			("subjectId", id.to_string()),
			("se", season.to_string()),
			("ep", episode.to_string()),
		],
	);
	let referer = absolute_url(&format!("/movies/{}", detail_path));
	let data = process_api(get_json(&url, Some(&referer)).await?)?;

	let downloads = data
		.get("downloads")
		.and_then(Value::as_array)
		.cloned()
		.unwrap_or_default();

	let mut streams: Vec<(i64, Value)> = downloads
		.iter()
		.filter_map(|download| {
			let link = download
				.get("url")
				.and_then(Value::as_str)
				.filter(|url| !url.is_empty())?;
			let resolution = download
				.get("resolution")
				.and_then(Value::as_i64)
				.unwrap_or(0);
			Some((
				resolution,
				json!({
					"url": link,
					"quality": format!("{resolution}p"),
					"codec": "",
					"size": lenient_u64(download.get("size")),
					"duration": 0,
				}),
			))
		})
		.collect();

	streams.sort_by_key(|(resolution, _)| *resolution);
	Ok(streams.into_iter().map(|(_, stream)| stream).collect())
}

// --- Dispatcher (contrat `source_server.py`) ------------------------------

fn query_value<'a>(query: &'a [(&str, String)], key: &str) -> Option<&'a str> {
	query
		.iter()
		.find(|(name, _)| *name == key)
		.map(|(_, value)| value.as_str())
}

fn query_i64(query: &[(&str, String)], key: &str, default: i64) -> i64 {
	query_value(query, key)
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(default)
}

/// Point d'entrée unique : traduit un chemin du service Python en JSON natif.
pub async fn source(path: &str, query: &[(&str, String)]) -> Result<Value> {
	let path = path.trim();

	if path == "/search" {
		let keyword = query_value(query, "q").unwrap_or("").trim();
		if keyword.is_empty() {
			return Ok(json!({ "code": 0, "data": { "items": [] } }));
		}
		let page = query_i64(query, "page", 1);
		let items = fetch_search(keyword, page, SEARCH_PER_PAGE).await?;
		let cards: Vec<Value> = items.iter().filter_map(as_card).collect();
		remember(&cards);
		return Ok(json!({ "code": 0, "data": { "items": cards } }));
	}

	if let Some(id) = path.strip_prefix("/detail/") {
		let res = fetch_res_data(id).await?;
		let data = detail_data(&res)?;
		return Ok(json!({ "code": 0, "data": data }));
	}

	if let Some(id) = path.strip_prefix("/episodes/") {
		let res = fetch_res_data(id).await?;
		let seasons = res
			.get("resData")
			.and_then(|res| res.get("resource"))
			.and_then(|resource| resource.get("seasons"))
			.and_then(Value::as_array)
			.cloned()
			.unwrap_or_default();
		// `maxEp` ne décrit pas toujours toutes les qualités : une saison peut
		// n'avoir que 10 épisodes en 480p et 22 en 360p. On transmet donc aussi
		// `allEp` et le détail `resolutions[].epNum` pour que la liste des épisodes
		// soit l'union de toutes les qualités.
		let list: Vec<Value> = seasons
			.iter()
			.map(|season| {
				let resolutions: Vec<Value> = season
					.get("resolutions")
					.and_then(Value::as_array)
					.map(|items| {
						items
							.iter()
							.filter_map(|resolution| {
								let ep_num = resolution.get("epNum").and_then(Value::as_i64)?;
								let quality = resolution.get("resolution").and_then(Value::as_i64)?;
								Some(json!({ "epNum": ep_num, "resolution": quality }))
							})
							.collect()
					})
					.unwrap_or_default();
				json!({
					"se": season.get("se").and_then(Value::as_i64).unwrap_or(0),
					"maxEp": season.get("maxEp").and_then(Value::as_i64).unwrap_or(0),
					"allEp": season.get("allEp").cloned().unwrap_or(Value::Null),
					"resolutions": resolutions,
				})
			})
			.collect();
		return Ok(json!({ "code": 0, "data": { "seasons": list } }));
	}

	if let Some(id) = path.strip_prefix("/stream/") {
		let season = query_i64(query, "season", 1);
		let episode = query_i64(query, "episode", 1);
		let streams = fetch_streams(id, season, episode).await?;
		return Ok(json!({ "code": 0, "data": { "streamList": streams } }));
	}

	// Rubriques (home/trending/movies/…).
	let collection = path.trim_matches('/');
	let page = query_i64(query, "page", 1);
	let items = match collection_query(collection) {
		None => fetch_trending(0, SEARCH_PER_PAGE).await?,
		Some(keyword) => fetch_search(keyword, page, SEARCH_PER_PAGE).await?,
	};
	let cards: Vec<Value> = items.iter().filter_map(as_card).collect();
	remember(&cards);
	Ok(json!({ "code": 0, "data": { "items": cards } }))
}

/// Fiche aplatie depuis `resData` (comme `source_server.detail`).
fn detail_data(res: &Value) -> Result<Value> {
	let subject = res
		.get("resData")
		.and_then(|res| res.get("subject"))
		.ok_or_else(|| ApiError::not_found("Fiche MovieBox introuvable"))?;

	let cover = subject
		.get("cover")
		.and_then(|cover| cover.get("url"))
		.and_then(Value::as_str)
		.map(|url| json!({ "url": url }));

	let stars = res
		.get("resData")
		.and_then(|res| res.get("stars"))
		.and_then(Value::as_array)
		.cloned()
		.unwrap_or_default();
	let staff_list: Vec<Value> = stars
		.iter()
		.map(|star| {
			json!({
				"name": star.get("name").cloned().unwrap_or(Value::Null),
				"avatarUrl": star.get("avatarUrl").cloned().unwrap_or(Value::Null),
			})
		})
		.collect();

	Ok(json!({
		"subjectId": string_of(subject.get("subjectId")).unwrap_or_default(),
		"subjectType": subject.get("subjectType").and_then(Value::as_i64).unwrap_or(0),
		"title": subject.get("title").and_then(Value::as_str).unwrap_or(""),
		"description": subject.get("description").and_then(Value::as_str).unwrap_or(""),
		"releaseDate": subject.get("releaseDate").cloned().unwrap_or(Value::Null),
		"duration": subject.get("duration").cloned().unwrap_or(Value::Null),
		"genre": split_csv(subject.get("genre")),
		"cover": cover,
		"imdbRatingValue": subject.get("imdbRatingValue").cloned().unwrap_or(Value::Null),
		"detailPath": subject.get("detailPath").and_then(Value::as_str).unwrap_or(""),
		"staffList": staff_list,
	}))
}

/// État de la source (pour `/api/health`).
pub async fn health() -> Value {
	let reachable = fetch_trending(0, 1).await.is_ok();
	json!({
		"enabled": true,
		"reachable": reachable,
		"baseUrl": web_base(),
		"upstream": { "status": if reachable { "ok" } else { "down" }, "service": "moviebox" },
	})
}
