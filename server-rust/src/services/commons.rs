//! Wikimedia Commons : seconde source de films gratuits (licences vérifiées par la communauté).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::cache::cache;
use crate::config::config;
use crate::error::{ApiError, Result};
use crate::http::{build_url, fetch_json, probe_remote_size};
use sha1::{Digest, Sha1};

use super::archive::{first_value, first_value_str, is_truthy, strip_html};

const ID_PREFIX: &str = "commons:";
const VARIANT_SEPARATOR: &str = "::";

static VIDEO_MIME: LazyLock<regex::Regex> =
	LazyLock::new(|| regex::Regex::new(r"(?i)^video/(webm|ogg|mp4|mpeg)$").unwrap());
static PLAYABLE_TRANSCODE: LazyLock<regex::Regex> =
	LazyLock::new(|| regex::Regex::new(r"(?i)^video/(webm|ogg|mp4|mpeg)").unwrap());
static VIDEO_EXTENSION: LazyLock<regex::Regex> =
	LazyLock::new(|| regex::Regex::new(r"(?i)\.(webm|ogv|ogg|mp4|mpg|mpeg)$").unwrap());
static SPACES: LazyLock<regex::Regex> =
	LazyLock::new(|| regex::Regex::new(r"\s+").unwrap());
static HTML_TAG: LazyLock<regex::Regex> =
	LazyLock::new(|| regex::Regex::new(r"<[^>]*>").unwrap());
static YEAR_RE: LazyLock<regex::Regex> =
	LazyLock::new(|| regex::Regex::new(r"(1[89]\d{2}|20\d{2})").unwrap());

/// Catégories vérifiées côté API (contiennent uniquement des fichiers vidéo).
pub const COMMONS_CATEGORIES: [(&str, &str); 5] = [
	("Films in the public domain", "Films du domaine public"),
	("Animated films", "Films d'animation"),
	("Silent films", "Films muets"),
	("Short films", "Courts-métrages"),
	("Documentary films", "Documentaires"),
];

pub const DEFAULT_COMMONS_CATEGORY: &str = "Films in the public domain";

pub fn is_commons_id(value: &str) -> bool {
	value.starts_with(ID_PREFIX)
}

pub fn commons_file_id(name: &str) -> String {
	format!("{ID_PREFIX}{name}")
}

pub fn commons_key(value: &str) -> &str {
	&value[ID_PREFIX.len()..]
}

/// Nom de fichier virtuel : `<fichier>` pour l'original, `<fichier>::<variante>` pour un transcodage.
pub fn commons_file_name(name: &str, variant: &str) -> String {
	if variant.is_empty() {
		name.to_string()
	} else {
		format!("{name}{VARIANT_SEPARATOR}{variant}")
	}
}

pub fn split_variant(file_name: &str) -> (String, String) {
	match file_name.split_once(VARIANT_SEPARATOR) {
		Some((name, variant)) => (name.to_string(), variant.to_string()),
		None => (file_name.to_string(), String::new()),
	}
}

fn sanitize(value: &str) -> String {
	let replaced = value.replace(['"', '\\'], " ");
	let collapsed = SPACES.replace_all(&replaced, " ");
	collapsed.trim().chars().take(80).collect()
}

/// File d'attente : un seul appel à la fois, avec un espacement minimal (limite de debit Wikimedia).
static QUEUE: LazyLock<tokio::sync::Mutex<()>> =
	LazyLock::new(|| tokio::sync::Mutex::new(()));
static LAST_CALL_MS: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as u64)
		.unwrap_or(0)
}

async fn commons_api(
	params: Vec<(String, String)>,
	timeout_ms: u64,
	retries: u32,
) -> Result<Value> {
	let _guard = QUEUE.lock().await;
	let _ = &_guard;

	let last = LAST_CALL_MS.load(Ordering::Relaxed);
	let elapsed = now_ms().saturating_sub(last);
	if elapsed < 300 {
		tokio::time::sleep(Duration::from_millis(300 - elapsed)).await;
	}

	let mut all_params: Vec<(String, String)> = vec![
		("format".to_string(), "json".to_string()),
		("origin".to_string(), "*".to_string()),
	];
	all_params.extend(params);

	let borrowed: Vec<(&str, String)> = all_params
		.iter()
		.map(|(key, value)| (key.as_str(), value.clone()))
		.collect();
	let url = build_url(&format!("{}/w/api.php", config().commons_api_url), &borrowed);
	let result = fetch_json(&url, timeout_ms, retries, &[]).await;
	LAST_CALL_MS.store(now_ms(), Ordering::Relaxed);
	result
}

const INFO_PROPS: [(&str, &str); 5] = [
	("prop", "videoinfo"),
	(
		"viprop",
		"url|size|mime|mediatype|derivatives|extmetadata|canonicaltitle",
	),
	("viurlwidth", "480"),
	(
		"viextmetadatafilter",
		"LicenseShortName|UsageTerms|LicenseUrl|Artist|ImageDescription|DateTimeOriginal|Credit|AttributionRequired",
	),
	("action", "query"),
];

fn info_props_params() -> Vec<(String, String)> {
	INFO_PROPS
		.iter()
		.map(|(key, value)| (key.to_string(), value.to_string()))
		.collect()
}

/// Titre lisible à partir du nom de fichier Commons.
fn title_from_name(name: &str) -> String {
	let without_ext = super::archive::base_name(name);
	let no_camera = regex::Regex::new(r"(?i)\s*\((?:16mm|8mm|35mm|TV|HD|UHD|SD|4K)[^)]*\)")
		.map(|re| re.replace_all(&without_ext, " ").to_string())
		.unwrap_or_else(|_| without_ext.clone());
	let no_dash = regex::Regex::new(r"(?i)\s*-\s*(?:16mm|8mm|35mm|silent|HD|UHD|flat|full)[^-]*$")
		.map(|re| re.replace_all(&no_camera, " ").to_string())
		.unwrap_or(no_camera);
	let underscores = no_dash.replace('_', " ");
	let collapsed = SPACES.replace_all(&underscores, " ");
	let cleaned = collapsed.trim().to_string();
	if cleaned.is_empty() {
		name.to_string()
	} else {
		cleaned
	}
}

fn year_from(value: Option<&Value>) -> Option<i64> {
	let text = first_value_str(value)?;
	YEAR_RE
		.captures(&text)
		.and_then(|captures| captures.get(1))
		.and_then(|found| found.as_str().parse().ok())
}

fn license_label_of(extmetadata: &Value) -> String {
	let short = strip_html(extmetadata.get("LicenseShortName").and_then(|v| v.get("value")));
	let usage = strip_html(
		extmetadata
			.get("UsageTerms")
			.and_then(|v| v.get("value")),
	)
	.to_lowercase();

	if usage.contains("public domain") || short.to_lowercase().contains("public domain") {
		return "Domaine public".to_string();
	}
	if short.to_lowercase().starts_with("cc0") {
		return "CC0 — domaine public".to_string();
	}
	if !short.is_empty() {
		return short;
	}
	"Licence libre (Wikimedia Commons)".to_string()
}

fn quality_of(height: i64, is_original: bool) -> &'static str {
	if height >= 720 {
		"HD"
	} else if height >= 480 {
		"SD"
	} else if is_original {
		"Original"
	} else {
		"SD"
	}
}

fn container_of(mime: &str) -> &'static str {
	let value = mime.to_lowercase();
	if value.contains("webm") {
		"WebM"
	} else if value.contains("ogg") {
		"Ogg"
	} else if value.contains("mp4") || value.contains("quicktime") || value.contains("mpeg4") {
		"MP4"
	} else {
		"Vidéo"
	}
}

/// Nom de fichier tel qu'il apparaît dans une URL Wikimedia (les espaces deviennent « _ »).
fn upload_name(name: &str) -> String {
	name.replace(' ', "_")
}

fn encode_upload_path(name: &str) -> String {
	upload_name(name)
		.split('/')
		.map(|segment| super::providers::urlencode(segment))
		.collect::<Vec<_>>()
		.join("/")
}

/// Chemin MD5 utilisé par Wikimedia : `/wikipedia/commons/<a>/<ab>/<fichier>`.
fn hash_dir(name: &str) -> String {
	let digest = format!("{:x}", md5::compute(upload_name(name)));
	format!("{}/{}", &digest[0..1], &digest[0..2])
}

pub fn commons_original_url(name: &str) -> String {
	format!(
		"{}/wikipedia/commons/{}/{}",
		config().commons_upload_url,
		hash_dir(name),
		encode_upload_path(name)
	)
}

fn commons_transcode_url(name: &str, variant: &str) -> String {
	format!(
		"{}/wikipedia/commons/transcoded/{}/{}/{}.{}",
		config().commons_upload_url,
		hash_dir(name),
		encode_upload_path(name),
		encode_upload_path(name),
		variant
	)
}

/// URLs confirmées par l'API : elles sont utilisées en priorité, la reconstruction étant un repli.
static KNOWN_URLS: LazyLock<Mutex<HashMap<String, String>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

fn remember_url(key: String, url: String) -> String {
	if !url.is_empty() {
		KNOWN_URLS.lock().unwrap().insert(key, url.clone());
	}
	url
}

/// Retire les paramètres de suivi ajoutés par l'API (`?utm_source=…`).
fn clean_remote_url(value: Option<&Value>) -> String {
	let raw = match value.and_then(|value| value.as_str()) {
		Some(raw) => raw,
		None => return String::new(),
	};
	let url = raw.split('?').next().unwrap_or("");
	if url.starts_with("https://") {
		url.to_string()
	} else {
		String::new()
	}
}

/// Taille réelle d'un fichier distant, mesurée par une requête `Range: bytes=0-0`.
async fn probe_remote_size_cached(url: &str) -> u64 {
	let key = format!("commons:size:{url}");
	match cache()
		.cached(&key, config().ttl.long, || async {
			let size = probe_remote_size(url, 15_000).await;
			Ok(Arc::new(Value::from(size)))
		})
		.await
	{
		Ok(value) => value.as_u64().unwrap_or(0),
		Err(_) => 0,
	}
}

/// Qualités disponibles : l'original + les transcodages WebM/MP4.
fn streams_from(name: &str, info: &Value) -> Vec<Value> {
	let original_height = info
		.get("height")
		.and_then(|value| value.as_i64())
		.unwrap_or(0);
	let original_cleaned = clean_remote_url(info.get("url"));
	let original_url = remember_url(
		commons_file_name(name, ""),
		if original_cleaned.is_empty() {
			commons_original_url(name)
		} else {
			original_cleaned
		},
	);

	let mut streams: Vec<(Value, i64, u64)> = vec![(
		json!({
			"name": commons_file_name(name, ""),
			"format": format!(
				"{}{}",
				container_of(info.get("mime").and_then(|value| value.as_str()).unwrap_or("")),
				if original_height > 0 {
					format!(" {original_height}p")
				} else {
					String::new()
				}
			),
			"size": info.get("size").and_then(|value| value.as_u64()).unwrap_or(0),
			"quality": quality_of(original_height, true),
			"url": original_url,
		}),
		if original_height >= 720 { 20 } else { 30 },
		info.get("size").and_then(|value| value.as_u64()).unwrap_or(0),
	)];

	if let Some(derivatives) = info.get("derivatives").and_then(|value| value.as_array()) {
		for derivative in derivatives {
			let variant = derivative
				.get("transcodekey")
				.and_then(|value| value.as_str())
				.unwrap_or("");
			if variant.is_empty() {
				continue;
			}
			let derivative_type = derivative
				.get("type")
				.and_then(|value| value.as_str())
				.unwrap_or("");
			if !PLAYABLE_TRANSCODE.is_match(derivative_type) {
				continue;
			}

			let height = derivative
				.get("height")
				.and_then(|value| value.as_i64())
				.unwrap_or(0);
			let cleaned = clean_remote_url(derivative.get("src"));
			let url = remember_url(
				commons_file_name(name, variant),
				if cleaned.is_empty() {
					commons_transcode_url(name, variant)
				} else {
					cleaned
				},
			);
			let size = derivative
				.get("size")
				.and_then(|value| match value {
					Value::Number(number) => number.as_u64(),
					Value::String(text) => text.parse().ok(),
					_ => None,
				})
				.unwrap_or(0);

			streams.push((
				json!({
					"name": commons_file_name(name, variant),
					"format": format!(
						"{}{}",
						container_of(derivative_type),
						if height > 0 { format!(" {height}p") } else { String::new() }
					),
					"size": size,
					"quality": quality_of(height, false),
					"url": url,
				}),
				if height >= 720 { 5 } else if height >= 480 { 10 } else { 15 },
				size,
			));
		}
	}

	streams.sort_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)));
	streams.into_iter().map(|(stream, _, _)| stream).collect()
}

fn file_name_of(page: &Value) -> String {
	page.get("title")
		.and_then(|value| value.as_str())
		.map(|title| title.strip_prefix("File:").unwrap_or(title).trim().to_string())
		.unwrap_or_default()
}

/// Carte normalisée, compatible avec les cartes Internet Archive utilisées par l'interface.
pub fn commons_card(page: &Value) -> Option<Value> {
	let info = page.get("videoinfo")?.as_array()?.first()?;
	let name = file_name_of(page);

	let mime = info.get("mime").and_then(|value| value.as_str()).unwrap_or("");
	if name.is_empty() || !VIDEO_MIME.is_match(mime) {
		return None;
	}

	let extmetadata = info.get("extmetadata").cloned().unwrap_or(json!({}));
	let description = strip_html(
		extmetadata
			.get("ImageDescription")
			.and_then(|value| value.get("value")),
	);
	let year = year_from(
		extmetadata
			.get("DateTimeOriginal")
			.and_then(|value| value.get("value")),
	)
	.or_else(|| year_from(Some(&Value::String(name.clone()))));

	let thumburl = info
		.get("thumburl")
		.and_then(|value| value.as_str())
		.map(Value::from)
		.unwrap_or(Value::Null);
	let duration = info.get("duration").and_then(|value| value.as_f64());
	let creator = strip_html(
		extmetadata.get("Artist").and_then(|value| value.get("value")),
	);
	let sha = Sha1::digest(name.as_bytes())
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect::<String>();

	Some(json!({
		"id": format!("commons-{}", &sha[..12]),
		"mediaType": "archive",
		"archiveId": commons_file_id(&name),
		"sourceId": commons_file_id(&name),
		"source": "commons",
		"sourceLabel": "Wikimedia Commons — domaine public / libre",
		"badge": "GRATUIT",
		"title": title_from_name(&name),
		"overview": description,
		"poster": thumburl.clone(),
		"thumb": thumburl.clone(),
		"thumbSmall": thumburl.clone(),
		"backdrop": thumburl.clone(),
		"backdropMobile": thumburl,
		"year": year.map(Value::from).unwrap_or(Value::Null),
		"rating": Value::Null,
		"downloads": Value::Null,
		"creator": if creator.is_empty() { Value::Null } else { Value::String(creator) },
		"license": extmetadata
			.get("LicenseUrl")
			.and_then(|value| value.get("value"))
			.cloned()
			.unwrap_or(Value::Null),
		"licenseLabel": Value::String(license_label_of(&extmetadata)),
		"collection": Value::Null,
		"language": Value::Null,
		"runtime": duration
		.map(|seconds| (seconds / 60.0).round() as i64)
		.map(Value::from)
		.unwrap_or(Value::Null),
		"duration": duration
			.map(|seconds| seconds as i64)
			.map(Value::from)
			.unwrap_or(Value::Null),
		"genres": json!([]),
		"genreIds": json!([]),
	}))
}

fn build_query(q: &str, category: &str) -> String {
	let mut parts = vec!["filetype:video".to_string()];
	let term = sanitize(q);
	if !term.is_empty() {
		parts.push(term);
	}
	if !category.is_empty() {
		parts.push(format!("incategory:\"{}\"", sanitize(category)));
	}
	parts.join(" ")
}

pub struct CommonsSearchParams<'a> {
	pub q: &'a str,
	pub category: &'a str,
	pub page: i64,
	pub rows: i64,
}

/// Recherche paginée (1 seul appel réseau par page) avec cache long.
pub async fn search_commons(params: CommonsSearchParams<'_>) -> Result<Arc<Value>> {
	let safe_page = params.page.max(1);
	let safe_rows = params.rows.clamp(1, 50);
	let active_category = if !params.category.is_empty() {
		params.category.to_string()
	} else if !sanitize(params.q).is_empty() {
		String::new()
	} else {
		DEFAULT_COMMONS_CATEGORY.to_string()
	};

	let cache_key = format!(
		"commons:search:{active_category}:{}:{safe_page}:{safe_rows}",
		params.q
	);
	let q = params.q.to_string();
	let rows = safe_rows;
	let offset = (safe_page - 1) * safe_rows;

	cache()
		.cached(&cache_key, config().ttl.medium, || async {
			let mut query_params: Vec<(String, String)> = vec![
				("generator".to_string(), "search".to_string()),
				(
					"gsrsearch".to_string(),
					build_query(&q, &active_category),
				),
				("gsrnamespace".to_string(), "6".to_string()),
				("gsrlimit".to_string(), rows.to_string()),
				("gsroffset".to_string(), offset.to_string()),
			];
			query_params.extend(info_props_params());

			let payload = commons_api(query_params, 10_000, 1).await?;
			let pages: Vec<Value> = payload
				.get("query")
				.and_then(|query| query.get("pages"))
				.and_then(|pages| pages.as_object())
				.map(|pages| pages.values().cloned().collect())
				.unwrap_or_default();

			let mut items: Vec<Value> = pages.iter().filter_map(commons_card).collect();
			items.sort_by_key(|item| {
				std::cmp::Reverse(
					item.get("duration")
						.and_then(|value| value.as_i64())
						.unwrap_or(0),
				)
			});

			let has_more = (items.len() as i64) >= rows;

			Ok(Arc::new(json!({
				"page": safe_page,
				"rows": rows,
				"license": "free",
				"total": Value::Null,
				"hasMore": has_more,
				"items": items,
			})))
		})
		.await
}

async fn fetch_info(file_names: &[String]) -> Result<Vec<Value>> {
	let unique: Vec<String> = file_names
		.iter()
		.filter(|name| !name.is_empty())
		.cloned()
		.collect::<std::collections::HashSet<String>>()
		.into_iter()
		.take(40)
		.collect();
	if unique.is_empty() {
		return Ok(vec![]);
	}

	let titles = unique
		.iter()
		.map(|name| format!("File:{name}"))
		.collect::<Vec<_>>()
		.join("|");
	let mut query_params: Vec<(String, String)> = vec![("titles".to_string(), titles)];
	query_params.extend(info_props_params());

	let payload = commons_api(query_params, 10_000, 1).await?;
	Ok(payload
		.get("query")
		.and_then(|query| query.get("pages"))
		.and_then(|pages| pages.as_object())
		.map(|pages| pages.values().cloned().collect())
		.unwrap_or_default())
}

async fn fetch_info_cached(name: &str) -> Result<Vec<Value>> {
	let key = format!("commons:item:{name}");
	let names = vec![name.to_string()];
	cache()
		.cached(&key, config().ttl.medium, || async {
			let pages = fetch_info(&names).await?;
			Ok(Arc::new(Value::Array(pages)))
		})
		.await
		.map(|value| value.as_array().cloned().unwrap_or_default())
}

/// Fiche complète d'un fichier Commons (métadonnées, licence et flux vidéo).
pub async fn get_commons_title(value: &str) -> Result<Value> {
	let name = commons_key(value).trim().to_string();
	if name.is_empty() || !VIDEO_EXTENSION.is_match(&name) {
		return Err(ApiError::bad_request("Fichier Wikimedia invalide"));
	}

	let pages = fetch_info_cached(&name).await?;
	let page = pages.first().cloned();
	let card = page.as_ref().and_then(commons_card);

	let Some(card) = card else {
		return Err(ApiError::not_found(
			"Ce fichier n'est plus proposé par Wikimedia Commons",
		));
	};

	let info = page
		.as_ref()
		.and_then(|page| {
			page.get("videoinfo")
				.and_then(|value| value.as_array())
				.and_then(|items| items.first())
		})
		.unwrap_or(&Value::Null);
	let streams = streams_from(&name, info);
	if streams.is_empty() {
		return Err(ApiError::new(
			422,
			"Aucun fichier vidéo lisible n'est disponible pour ce média",
		));
	}

	let mut result = card.as_object().cloned().unwrap_or_default();
	result.insert("subjects".to_string(), json!([]));
	result.insert("streams".to_string(), Value::Array(streams));
	result.insert("episodes".to_string(), json!([]));
	result.insert("kind".to_string(), Value::String("movie".to_string()));
	result.insert("embedUrl".to_string(), Value::Null);
	result.insert(
		"detailsUrl".to_string(),
		Value::String(format!(
			"{}/wiki/File:{}",
			config().commons_api_url,
			super::providers::urlencode(&name.replace(' ', "_"))
		)),
	);
	result.insert(
		"sourceLabel".to_string(),
		Value::String("Wikimedia Commons — domaine public / libre".to_string()),
	);

	Ok(Value::Object(result))
}

/// URL distante d'un fichier déjà validé (téléchargement / reprise).
pub fn commons_remote_url(identifier: &str, file_name: &str) -> Option<String> {
	let name = commons_key(identifier).trim();
	let (base_name, variant) = split_variant(file_name);
	if name.is_empty() || base_name != name {
		return None;
	}

	let key = if variant.is_empty() {
		commons_file_name(name, "")
	} else {
		commons_file_name(name, &variant)
	};
	if let Some(known) = KNOWN_URLS.lock().unwrap().get(&key) {
		return Some(known.clone());
	}

	Some(if variant.is_empty() {
		commons_original_url(name)
	} else {
		commons_transcode_url(name, &variant)
	})
}

/// Valide un couple (identifiant Commons, nom de fichier) avant téléchargement.
pub async fn get_commons_file(identifier: &str, file_name: &str) -> Result<Value> {
	let name = commons_key(identifier).trim().to_string();
	let (base_name, variant) = split_variant(file_name);

	if name.is_empty() || !VIDEO_EXTENSION.is_match(&name) {
		return Err(ApiError::bad_request("Identifiant Wikimedia invalide"));
	}
	if base_name != name {
		return Err(ApiError::bad_request("Nom de fichier invalide"));
	}
	if !variant.is_empty() {
		let valid = !variant.is_empty()
			&& variant.len() <= 40
			&& variant
				.chars()
				.all(|c| c.is_ascii_alphanumeric() || c == '.');
		if !valid {
			return Err(ApiError::bad_request("Variante de fichier invalide"));
		}
	}

	let pages = fetch_info_cached(&name).await?;
	let info = pages
		.first()
		.and_then(|page| {
			page.get("videoinfo")
				.and_then(|value| value.as_array())
				.and_then(|items| items.first())
		})
		.cloned();

	let Some(info) = info else {
		return Err(ApiError::not_found(
			"Ce fichier n'existe pas sur Wikimedia Commons",
		));
	};

	let wanted = if variant.is_empty() {
		commons_file_name(&name, "")
	} else {
		commons_file_name(&name, &variant)
	};
	let streams = streams_from(&name, &info);
	let stream = streams
		.iter()
		.find(|item| item.get("name").and_then(|value| value.as_str()) == Some(wanted.as_str()));

	let Some(stream) = stream else {
		return Err(ApiError::not_found(
			"Cette qualité n'est pas disponible pour ce fichier",
		));
	};

	// L'API ne donne la taille que des transcodages rarement : on mesure si nécessaire.
	let size = stream
		.get("size")
		.and_then(|value| value.as_u64())
		.filter(|size| *size > 0)
		.unwrap_or_else(|| 0);
	let size = if size > 0 {
		size
	} else {
		probe_remote_size_cached(
			stream
				.get("url")
				.and_then(|value| value.as_str())
				.unwrap_or(""),
		)
		.await
	};

	Ok(json!({
		"identifier": commons_file_id(&name),
		"name": stream.get("name").cloned().unwrap_or(Value::Null),
		"size": size,
		"format": stream.get("format").cloned().unwrap_or(Value::String("Vidéo".to_string())),
		"url": stream.get("url").cloned().unwrap_or(Value::Null),
	}))
}
