//! Internet Archive : recherche filtrée sur les droits déclarés + résolution des fichiers vidéo.

use std::sync::{Arc, LazyLock};

use regex::Regex;
use serde_json::{json, Value};

use crate::cache::cache;
use crate::config::config;
use crate::error::{ApiError, Result};
use crate::http::{build_url, fetch_json};
use crate::services::providers::urlencode;

static VIDEO_EXTENSIONS: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)\.(mp4|m4v|webm|ogv|mkv|avi|mpeg|mpg|mov)$").unwrap());
static SANITIZE_BAD: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}\s'-]+").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static HTML_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static YEAR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(1[89]\d{2}|20\d{2})").unwrap());
static CLOCK_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"^(\d{1,2}):(\d{2})(?::(\d{2}))?$").unwrap());
static HOURS_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)(\d+)\s*h(?:eures?)?\s*(\d+)?").unwrap());
static MINUTES_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)(\d+)\s*(min|minutes?|mn)").unwrap());
static ONLY_NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)$").unwrap());
static EXT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.[a-z0-9]{2,5}$").unwrap());
static MP4_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.(mp4|m4v)$").unwrap());
static HD_NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"1080|720").unwrap());
static HD_FORMAT_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)h\.264 ia|hd|hires").unwrap());
static SD_FORMAT_RE: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)512kb|mpeg4|mpeg2|webm|ogg").unwrap());
static QUALITY_NOISE: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(
		r"(?i)\b(512\s?kb|1\s?mb|hq|hd|sd|4k|1080p|720p|480p|360p|full|remastered|restored|ia)\b",
	)
	.unwrap()
});
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]+").unwrap());
static SEASON_PATTERN: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)\bs(?:aison|eason)?[\s._-]*(\d{1,2})\b").unwrap());

const FIELDS: [&str; 10] = [
	"identifier",
	"title",
	"year",
	"date",
	"downloads",
	"description",
	"creator",
	"licenseurl",
	"collection",
	"language",
];

const FORMAT_PRIORITY: [&str; 9] = [
	"h.264 ia",
	"h.264 hd",
	"hires mpeg4",
	"h.264",
	"mpeg4",
	"512kb mpeg4",
	"mpeg2",
	"webm",
	"ogg video",
];

pub const ARCHIVE_LICENSES: [(&str, &str, &str); 4] = [
	(
		"free",
		"Domaine public & Creative Commons",
		"(licenseurl:*creativecommons* OR licenseurl:*publicdomain*)",
	),
	(
		"publicdomain",
		"Domaine public uniquement",
		"licenseurl:*publicdomain*",
	),
	(
		"cc",
		"Creative Commons (hors domaine public)",
		"(licenseurl:*creativecommons* AND NOT licenseurl:*publicdomain*)",
	),
	("all", "Toutes les archives (droits non verifies)", ""),
];

pub const DEFAULT_ARCHIVE_LICENSE: &str = "free";

pub const ARCHIVE_COLLECTIONS: [(&str, &str); 12] = [
	("feature_films", "Longs-métrages libres"),
	("moviesandfilms", "Films populaires du web"),
	("classic_tv", "Séries TV classiques"),
	("television", "Télévision & émissions"),
	("Film_Noir", "Cinéma noir"),
	("SciFi_Horror", "Science-fiction & horreur"),
	("silent_films", "Films muets"),
	("animationandcartoons", "Animation"),
	("classic_cartoons", "Dessins animés"),
	("short_films", "Courts-métrages"),
	("prelinger", "Archives Prelinger"),
	("opensource_movies", "Vidéos libres (open source)"),
];

pub const DEFAULT_ARCHIVE_COLLECTION: &str = "feature_films";

pub fn resolve_archive_license(value: Option<&str>) -> (&'static str, &'static str, &'static str) {
	ARCHIVE_LICENSES
		.iter()
		.find(|(id, _, _)| Some(*id) == value)
		.or_else(|| ARCHIVE_LICENSES.iter().find(|(id, _, _)| *id == DEFAULT_ARCHIVE_LICENSE))
		.copied()
		.unwrap()
}

pub fn resolve_archive_sort(value: Option<&str>) -> &'static str {
	match value {
		Some("recent") => "date desc",
		Some("oldest") => "date asc",
		Some("title") => "title asc",
		Some("rating") => "avg_rating desc",
		_ => "downloads desc",
	}
}

/// Premier élément « truthy » d'une valeur (scalaire ou tableau), comme en JS.
pub fn first_value(value: Option<&Value>) -> Option<&Value> {
	match value? {
		Value::Array(items) => items.iter().find(|item| is_truthy(item)),
		value if is_truthy(value) => Some(value),
		_ => None,
	}
}

pub fn is_truthy(value: &Value) -> bool {
	match value {
		Value::Null => false,
		Value::Bool(value) => *value,
		Value::Number(value) => value.as_f64().unwrap_or(0.0) != 0.0,
		Value::String(value) => !value.is_empty(),
		Value::Array(items) => !items.is_empty(),
		_ => true,
	}
}

pub fn first_value_str(value: Option<&Value>) -> Option<String> {
	match first_value(value)? {
		Value::String(value) => Some(value.clone()),
		Value::Number(value) => Some(value.to_string()),
		Value::Bool(value) => Some(value.to_string()),
		_ => None,
	}
}

pub fn strip_html(value: Option<&Value>) -> String {
	let raw = match value {
		Some(Value::Array(items)) => items
			.iter()
			.filter_map(|item| item.as_str())
			.collect::<Vec<_>>()
			.join(" "),
		Some(Value::String(value)) => value.clone(),
		Some(Value::Number(value)) => value.to_string(),
		_ => String::new(),
	};

	let without_tags = HTML_TAG.replace_all(&raw, " ");
	let decoded = without_tags
		.replace("&amp;", "&")
		.replace("&quot;", "\"")
		.replace("&#39;", "'")
		.replace("&nbsp;", " ");
	SPACES.replace_all(&decoded, " ").trim().to_string()
}

pub fn base_name(name: &str) -> String {
	EXT_RE.replace(name, "").to_string()
}

pub fn parse_year(value: Option<&Value>) -> Option<i64> {
	let text = first_value_str(value)?;
	YEAR_RE
		.captures(&text)
		.and_then(|captures| captures.get(1))
		.and_then(|found| found.as_str().parse().ok())
}

pub fn parse_runtime_minutes(value: Option<&Value>) -> Option<i64> {
	let raw = first_value_str(value)?;
	let raw = raw.trim();
	if raw.is_empty() {
		return None;
	}

	if let Some(captures) = CLOCK_RE.captures(raw) {
		let hours: i64 = captures.get(1)?.as_str().parse().ok()?;
		let minutes: i64 = captures.get(2)?.as_str().parse().ok()?;
		return Some(hours * 60 + minutes);
	}

	if let Some(captures) = HOURS_RE.captures(raw) {
		let hours: i64 = captures.get(1)?.as_str().parse().ok()?;
		let minutes: i64 = captures
			.get(2)
			.and_then(|found| found.as_str().parse().ok())
			.unwrap_or(0);
		return Some(hours * 60 + minutes);
	}

	if let Some(captures) = MINUTES_RE.captures(raw) {
		return captures.get(1)?.as_str().parse().ok();
	}
	if let Some(captures) = ONLY_NUM_RE.captures(raw) {
		return captures.get(1)?.as_str().parse().ok();
	}

	None
}

fn sanitize_term(value: &str) -> String {
	let replaced = SANITIZE_BAD.replace_all(value, " ");
	let collapsed = SPACES.replace_all(&replaced, " ");
	collapsed.trim().chars().take(80).collect()
}

/// Libellé lisible d'une licence à partir de son URL (affichage sur les fiches).
fn license_label_of(url: Option<&str>) -> Option<String> {
	let value = first_value_str(url.map(Value::from).as_ref())?.to_lowercase();
	if value.is_empty() {
		return None;
	}
	if value.contains("publicdomain/zero") {
		return Some("CC0 — domaine public".to_string());
	}
	if value.contains("publicdomain/mark") || value.contains("/licenses/publicdomain/") {
		return Some("Domaine public".to_string());
	}
	if value.contains("/by-nc-sa/") {
		return Some("CC BY-NC-SA".to_string());
	}
	if value.contains("/by-nc-nd/") {
		return Some("CC BY-NC-ND".to_string());
	}
	if value.contains("/by-nc/") {
		return Some("CC BY-NC".to_string());
	}
	if value.contains("/by-sa/") {
		return Some("CC BY-SA".to_string());
	}
	if value.contains("/by-nd/") {
		return Some("CC BY-ND".to_string());
	}
	if value.contains("/by/") {
		return Some("CC BY".to_string());
	}
	Some("Licence libre (Creative Commons)".to_string())
}

/// Carte normalisée d'une archive (compatible avec les cartes du front).
pub fn archive_card(doc: &Value) -> Option<Value> {
	let identifier = doc.get("identifier")?.as_str()?;

	let title = first_value_str(doc.get("title")).unwrap_or_else(|| identifier.to_string());
	let poster = format!("{}/services/img/{}", config().archive_base_url, identifier);
	let license = first_value_str(doc.get("licenseurl"));

	Some(json!({
		"id": format!("archive-{identifier}"),
		"mediaType": "archive",
		"archiveId": identifier,
		"source": "archive",
		"badge": "GRATUIT",
		"title": title,
		"overview": strip_html(doc.get("description")),
		"poster": poster.clone(),
		"thumb": poster.clone(),
		"thumbSmall": poster.clone(),
		"backdrop": poster.clone(),
		"backdropMobile": poster,
		"year": parse_year(doc.get("year").or(doc.get("date"))).map(Value::from).unwrap_or(Value::Null),
		"rating": Value::Null,
		"downloads": doc.get("downloads").and_then(|value| value.as_i64()).unwrap_or(0),
		"creator": first_value_str(doc.get("creator")).map(Value::String).unwrap_or(Value::Null),
		"license": license.clone().map(Value::String).unwrap_or(Value::Null),
		"licenseLabel": license_label_of(license.as_deref()).map(Value::String).unwrap_or(Value::Null),
		"collection": first_value_str(doc.get("collection")).map(Value::String).unwrap_or(Value::Null),
		"language": first_value_str(doc.get("language")).map(Value::String).unwrap_or(Value::Null),
		"runtime": parse_runtime_minutes(doc.get("runtime")).map(Value::from).unwrap_or(Value::Null),
		"genres": json!([]),
		"genreIds": json!([]),
	}))
}

fn build_search_query(q: &str, collection: &str, license: &str) -> String {
	let mut clauses = vec!["mediatype:(movies)".to_string()];
	if !collection.is_empty() {
		clauses.push(format!("collection:({collection})"));
	}

	// Le filtre de droits s'applique à toutes les requêtes.
	let (_, _, clause) = resolve_archive_license(Some(license));
	if !clause.is_empty() {
		clauses.push(clause.to_string());
	}

	let term = sanitize_term(q);
	if !term.is_empty() {
		clauses.push(format!("(title:\"{term}\" OR description:\"{term}\")"));
	}

	clauses.join(" AND ")
}

pub struct ArchiveSearchParams<'a> {
	pub q: &'a str,
	pub collection: &'a str,
	pub license: &'a str,
	pub page: i64,
	pub rows: i64,
	pub sort: Option<&'a str>,
	pub timeout_ms: u64,
	pub retries: u32,
}

/// Recherche paginée avec cache : la page d'accueil a un budget global.
pub async fn search_archive(params: ArchiveSearchParams<'_>) -> Result<Arc<Value>> {
	let safe_page = params.page.max(1);
	let safe_rows = params.rows.clamp(1, 60);
	let active_license = resolve_archive_license(Some(params.license));
	let cache_key = format!(
		"archive:search:{}:{}:{}:{}:{}:{}",
		params.q,
		params.collection,
		active_license.0,
		safe_page,
		safe_rows,
		params.sort.unwrap_or("popular")
	);

	let q = params.q.to_string();
	let collection = params.collection.to_string();
	let license_id = active_license.0.to_string();
	let timeout_ms = params.timeout_ms;
	let retries = params.retries;
	let sort = resolve_archive_sort(params.sort);

	cache()
		.cached(&cache_key, config().ttl.medium, || async {
			let mut pairs: Vec<(&str, String)> = vec![
				("q", build_search_query(&q, &collection, &license_id)),
				("rows", safe_rows.to_string()),
				("page", safe_page.to_string()),
				("output", "json".to_string()),
			];
			for field in FIELDS {
				pairs.push(("fl[]", field.to_string()));
			}
			pairs.push(("sort[]", sort.to_string()));

			let url = build_url(
				&format!("{}/advancedsearch.php", config().archive_base_url),
				&pairs,
			);
			let payload = fetch_json(&url, timeout_ms, retries, &[]).await?;
			let response = payload.get("response").cloned().unwrap_or(json!({}));

			let total = response
				.get("numFound")
				.and_then(|value| value.as_i64())
				.unwrap_or(0);
			let items: Vec<Value> = response
				.get("docs")
				.and_then(|value| value.as_array())
				.map(|docs| docs.iter().filter_map(archive_card).collect())
				.unwrap_or_default();

			Ok(Arc::new(json!({
				"page": safe_page,
				"rows": safe_rows,
				"license": license_id,
				"total": total,
				"hasMore": i64::from(safe_page) * safe_rows < total,
				"items": items,
			})))
		})
		.await
}

/// Référence d'épisode détectée dans un nom de fichier.
struct EpisodeRef {
	season: Option<i64>,
	number: i64,
}

static EPISODE_PATTERNS: LazyLock<Vec<(Regex, Option<usize>, usize)>> = LazyLock::new(|| {
	vec![
		(
			Regex::new(r"(?i)\bS(\d{1,2})[\s._-]*E(\d{1,3})\b").unwrap(),
			Some(1),
			2,
		),
		(
			Regex::new(r"(?i)\b(\d{1,2})\s?x(\d{1,3})\b").unwrap(),
			Some(1),
			2,
		),
		(
			Regex::new(
				r"(?i)\b(?:ep|episode|part|pt|chapter|cap[ií]tulo|capitulo|folge|episodio|epis[oó]dio)[\s._-]*(\d{1,3})\b",
			)
			.unwrap(),
			None,
			1,
		),
		(Regex::new(r"[-–—]\s*(\d{1,3})\s*[-–—]").unwrap(), None, 1),
		(Regex::new(r"^\s*(\d{1,3})[\s._-]+[^\d]").unwrap(), None, 1),
	]
});

pub fn episode_label(season: Option<i64>, number: i64) -> String {
	match season {
		Some(season) => format!("S{:02}E{:02}", season, number),
		None => format!("Épisode {number}"),
	}
}

/// Nom lisible d'un épisode (sans dossier, ni extension, ni bruit de qualité).
fn episode_name(name: &str) -> String {
	let leaf = name.rsplit('/').next().unwrap_or(name);
	let base = base_name(leaf);
	let without_noise = QUALITY_NOISE.replace_all(&base, " ");
	let spaced = SPACES.replace_all(&without_noise, " ");
	let cleaned = spaced
		.trim()
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ");
	if cleaned.is_empty() {
		base
	} else {
		cleaned
	}
}

fn normalized_name(name: &str) -> String {
	let base = base_name(name);
	let without_noise = QUALITY_NOISE.replace_all(&base, " ");
	let only_alnum = NON_ALNUM.replace_all(&without_noise, " ");
	only_alnum.trim().to_lowercase()
}

/// Détecte saison et numéro d'épisode dans un nom de fichier (S01E02, 1x02, « Ep 3 »…).
fn episode_from_name(name: &str) -> Option<EpisodeRef> {
	let base = base_name(name);

	for (pattern, season_group, number_group) in EPISODE_PATTERNS.iter() {
		let Some(captures) = pattern.captures(&base) else {
			continue;
		};
		let Some(number_capture) = captures.get(*number_group) else {
			continue;
		};
		let Ok(number) = number_capture.as_str().parse::<i64>() else {
			continue;
		};
		if number <= 0 {
			continue;
		}

		let season = season_group
			.and_then(|index| captures.get(index))
			.and_then(|found| found.as_str().parse::<i64>().ok());
		if let Some(season) = season {
			if season > 0 {
				return Some(EpisodeRef {
					season: Some(season),
					number,
				});
			}
		}

		// Nombreux items Internet Archive rangent les épisodes dans un dossier « Season 3 ».
		let folder_season = SEASON_PATTERN
			.captures(&base)
			.and_then(|captures| captures.get(1))
			.and_then(|found| found.as_str().parse::<i64>().ok())
			.filter(|season| *season > 0);

		return Some(EpisodeRef {
			season: folder_season,
			number,
		});
	}

	None
}

struct AnalyzedStream {
	name: String,
	format: String,
	size: u64,
	quality: String,
	url: String,
	score: u32,
	group_key: String,
	episode: Option<EpisodeRef>,
}

fn stream_json(stream: &AnalyzedStream) -> Value {
	json!({
		"name": stream.name,
		"format": stream.format,
		"size": stream.size,
		"quality": stream.quality,
		"url": stream.url,
	})
}

/// Analyse les fichiers vidéo d'une archive : une entrée = un fichier vidéo.
fn analyze_files(identifier: &str, files: &[Value]) -> Vec<AnalyzedStream> {
	let mut analyzed: Vec<AnalyzedStream> = files
		.iter()
		.filter_map(|file| {
			let name = file.get("name")?.as_str()?;
			if !VIDEO_EXTENSIONS.is_match(name) {
				return None;
			}

			let format = file
				.get("format")
				.and_then(|value| value.as_str())
				.unwrap_or("")
				.to_string();
			let priority = FORMAT_PRIORITY
				.iter()
				.position(|value| value.eq_ignore_ascii_case(&format));
			let is_mp4 = MP4_RE.is_match(name);
			let quality = if HD_FORMAT_RE.is_match(&format) || HD_NAME_RE.is_match(name) {
				"HD"
			} else if SD_FORMAT_RE.is_match(&format) || is_mp4 {
				"SD"
			} else {
				"Original"
			};
			let episode = episode_from_name(name);
			let score = if is_mp4 { 0 } else { 10 } + priority.unwrap_or(20) as u32;

			let group_key = match &episode {
				Some(episode) => format!(
					"episode:{}:{}",
					episode
						.season
						.map(|value| value.to_string())
						.unwrap_or_else(|| "x".to_string()),
					episode.number
				),
				None => format!("file:{}", normalized_name(name)),
			};

			let size = file
				.get("size")
				.and_then(|value| match value {
					Value::Number(number) => number.as_u64(),
					Value::String(text) => text.parse().ok(),
					_ => None,
				})
				.unwrap_or(0);

			Some(AnalyzedStream {
				name: name.to_string(),
				format: if format.is_empty() {
					"Vidéo".to_string()
				} else {
					format
				},
				size,
				quality: quality.to_string(),
				url: format!(
					"{}/download/{}/{}",
					config().archive_base_url,
					urlencode(identifier),
					urlencode(name)
				),
				score,
				group_key,
				episode,
			})
		})
		.collect();

	analyzed.sort_by(|a, b| a.score.cmp(&b.score).then(b.size.cmp(&a.size)));
	analyzed
}

pub struct Library {
	pub streams: Vec<Value>,
	pub episodes: Vec<Value>,
	pub kind: &'static str,
}

/// Construit la bibliothèque lisible d'une archive (qualités + épisodes détectés).
fn build_library(identifier: &str, files: &[Value]) -> Library {
	let analyzed = analyze_files(identifier, files);
	let streams: Vec<Value> = analyzed.iter().map(stream_json).collect();

	if streams.is_empty() {
		return Library {
			streams,
			episodes: vec![],
			kind: "movie",
		};
	}

	// Regroupement par clé, dans l'ordre trié (meilleur score d'abord).
	let mut order: Vec<String> = Vec::new();
	let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
	for (index, item) in analyzed.iter().enumerate() {
		let entry = groups.entry(item.group_key.clone()).or_insert_with(Vec::new);
		if entry.is_empty() {
			order.push(item.group_key.clone());
		}
		entry.push(index);
	}

	let mut episodes: Vec<Value> = order
		.iter()
		.filter_map(|key| {
			let indexes = groups.get(key)?;
			let best = &analyzed[indexes[0]];
			let episode = best.episode.as_ref()?;
			Some(json!({
				"season": episode.season.map(Value::from).unwrap_or(Value::Null),
				"number": episode.number,
				"label": episode_label(episode.season, episode.number),
				"name": episode_name(&best.name),
				"streams": indexes.iter().map(|index| stream_json(&analyzed[*index])).collect::<Vec<_>>(),
			}))
		})
		.collect();

	episodes.sort_by_key(|episode| {
		let season = episode
			.get("season")
			.and_then(|value| value.as_i64())
			.unwrap_or(0);
		let number = episode
			.get("number")
			.and_then(|value| value.as_i64())
			.unwrap_or(0);
		(season, number)
	});

	Library {
		kind: if episodes.len() > 1 { "series" } else { "movie" },
		streams,
		episodes,
	}
}

async fn fetch_archive_item(identifier: &str) -> Result<Arc<Value>> {
	let key = format!("archive:item:{identifier}");
	cache()
		.cached(&key, config().ttl.medium, || async {
			let url = format!(
				"{}/metadata/{}",
				config().archive_base_url,
				urlencode(identifier)
			);
			let payload = fetch_json(&url, 15_000, 1, &[]).await?;
			Ok(Arc::new(payload))
		})
		.await
}

/// Fiche complète d'une archive (métadonnées + flux + épisodes).
pub async fn get_archive_title(identifier: &str) -> Result<Value> {
	if identifier.is_empty() {
		return Err(crate::error::ApiError::not_found("Film introuvable"));
	}

	let payload = fetch_archive_item(identifier).await?;
	let is_dark = first_value(payload.get("is_dark")).is_some()
		|| payload
			.get("metadata")
			.and_then(|metadata| metadata.get("is_dark"))
			.and_then(|value| value.as_str())
			== Some("true");
	if is_dark {
		return Err(crate::error::ApiError::not_found(
			"Cette vidéo n'est plus proposée par Internet Archive",
		));
	}

	let metadata = payload.get("metadata").cloned().unwrap_or(json!({}));
	let files: Vec<Value> = payload
		.get("files")
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();
	let library = build_library(identifier, &files);

	if library.streams.is_empty() {
		return Err(crate::error::ApiError::new(
			422,
			"Aucun fichier vidéo lisible n'est disponible pour cette archive",
		));
	}

	let doc = json!({
		"identifier": identifier,
		"title": metadata.get("title").cloned().unwrap_or(Value::Null),
		"year": metadata.get("year").or(metadata.get("date")).cloned().unwrap_or(Value::Null),
		"downloads": metadata.get("downloads").cloned().unwrap_or(Value::Null),
		"description": metadata.get("description").cloned().unwrap_or(Value::Null),
		"creator": metadata.get("creator").cloned().unwrap_or(Value::Null),
		"licenseurl": metadata.get("licenseurl").cloned().unwrap_or(Value::Null),
		"collection": metadata.get("collection").cloned().unwrap_or(Value::Null),
		"language": metadata.get("language").cloned().unwrap_or(Value::Null),
		"runtime": metadata.get("runtime").cloned().unwrap_or(Value::Null),
	});
	let card = archive_card(&doc).unwrap_or(json!({}));

	let overview = match card.get("overview").and_then(|value| value.as_str()) {
		Some(value) if !value.is_empty() => value.to_string(),
		_ => strip_html(metadata.get("description")),
	};

	let subjects: Vec<Value> = match metadata.get("subject") {
		Some(Value::Array(items)) => items
			.iter()
			.filter(|item| is_truthy(item))
			.take(8)
			.cloned()
			.collect(),
		Some(value) if is_truthy(value) => vec![value.clone()],
		_ => vec![],
	};

	let mut result = card.as_object().cloned().unwrap_or_default();
	result.insert("overview".to_string(), Value::String(overview));
	result.insert("subjects".to_string(), Value::Array(subjects));
	result.insert("streams".to_string(), Value::Array(library.streams));
	result.insert("episodes".to_string(), Value::Array(library.episodes));
	result.insert("kind".to_string(), Value::String(library.kind.to_string()));
	result.insert(
		"embedUrl".to_string(),
		Value::String(format!(
			"{}/embed/{}",
			config().archive_base_url,
			urlencode(identifier)
		)),
	);
	result.insert(
		"detailsUrl".to_string(),
		Value::String(format!(
			"{}/details/{}",
			config().archive_base_url,
			urlencode(identifier)
		)),
	);
	result.insert(
		"sourceLabel".to_string(),
		Value::String("Internet Archive — domaine public / libre".to_string()),
	);

	Ok(Value::Object(result))
}

/// Valide un fichier vidéo d'une archive avant tout téléchargement.
pub async fn get_archive_file(identifier: &str, file_name: &str) -> Result<Value> {
	let clean_id = identifier.trim();
	let clean_name = file_name.trim();

	let id_ok = !clean_id.is_empty()
		&& clean_id.len() <= 120
		&& clean_id
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-');
	if !id_ok {
		return Err(crate::error::ApiError::bad_request(
			"Identifiant d'archive invalide",
		));
	}
	if clean_name.is_empty()
		|| clean_name.len() > 200
		|| clean_name.contains('/')
		|| clean_name.contains('\\')
	{
		return Err(crate::error::ApiError::bad_request("Nom de fichier invalide"));
	}

	let payload = fetch_archive_item(clean_id).await?;
	let file = payload
		.get("files")
		.and_then(|value| value.as_array())
		.and_then(|files| {
			files.iter().find(|entry| {
				entry.get("name").and_then(|value| value.as_str()) == Some(clean_name)
			})
		});

	let Some(file) = file else {
		return Err(crate::error::ApiError::not_found(
			"Ce fichier n'existe pas dans cette archive",
		));
	};
	if !VIDEO_EXTENSIONS.is_match(clean_name) {
		return Err(crate::error::ApiError::bad_request(
			"Ce fichier n'est pas une vidéo lisible",
		));
	}

	Ok(json!({
		"identifier": clean_id,
		"name": clean_name,
		"size": file.get("size").and_then(|value| match value {
			Value::Number(number) => number.as_u64(),
			Value::String(text) => text.parse().ok(),
			_ => None,
		}).unwrap_or(0),
		"format": file.get("format").and_then(|value| value.as_str()).unwrap_or("Vidéo"),
		"url": format!(
			"{}/download/{}/{}",
			config().archive_base_url,
			urlencode(clean_id),
			urlencode(clean_name)
		),
	}))
}

/// Titres liés (même collection, sans l'élément courant).
pub async fn get_related_archive(identifier: &str, collection: &str, rows: i64) -> Result<Value> {
	let result = search_archive(ArchiveSearchParams {
		q: "",
		collection,
		license: DEFAULT_ARCHIVE_LICENSE,
		page: 1,
		rows: rows + 1,
		sort: Some("popular"),
		timeout_ms: 10_000,
		retries: 1,
	})
	.await?;

	let mut items: Vec<Value> = result
		.get("items")
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();
	items.retain(|item| item.get("archiveId").and_then(|value| value.as_str()) != Some(identifier));
	items.truncate(rows as usize);

	let mut output = result.as_object().cloned().unwrap_or_default();
	output.insert("items".to_string(), Value::Array(items));
	Ok(Value::Object(output))
}
