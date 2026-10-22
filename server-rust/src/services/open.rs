//! Catalogues libres supplémentaires : NASA, Blender Open Movies, Europeana,
//! Pexels, Pixabay et Library of Congress.
//!
//! Les sources à clé (Europeana, Pexels, Pixabay) ne sont proposées que si la
//! clé correspondante est configurée. Library of Congress reste désactivée par
//! défaut (`LOC_API_ENABLED=true` pour l'activer) car son API JSON est parfois
//! protégée par Cloudflare.

use std::sync::{Arc, LazyLock};

use regex::Regex;
use serde_json::{json, Value};

use crate::cache::cache;
use crate::config::config;
use crate::error::{ApiError, Result};
use crate::http::{build_url, fetch_json};
use crate::services::providers::urlencode;

use super::archive::{first_value, first_value_str, strip_html};

pub const NASA: &str = "nasa";
pub const BLENDER: &str = "blender";
pub const EUROPEANA: &str = "europeana";
pub const PEXELS: &str = "pexels";
pub const PIXABAY: &str = "pixabay";
pub const LOC: &str = "loc";

static VIDEO_EXT: LazyLock<Regex> =
	LazyLock::new(|| Regex::new(r"(?i)\.(mp4|m4v|webm|ogv|mov|mkv)$").unwrap());

/// Source statique reconnue (utilisée par les routes /free).
pub fn resolve_source(value: &str) -> Option<&'static str> {
	match value {
		NASA => Some(NASA),
		BLENDER => Some(BLENDER),
		EUROPEANA if enabled(EUROPEANA) => Some(EUROPEANA),
		PEXELS if enabled(PEXELS) => Some(PEXELS),
		PIXABAY if enabled(PIXABAY) => Some(PIXABAY),
		LOC if enabled(LOC) => Some(LOC),
		_ => None,
	}
}

pub fn enabled(source: &str) -> bool {
	match source {
		NASA | BLENDER => true,
		EUROPEANA => !config().europeana_api_key.trim().is_empty(),
		PEXELS => !config().pexels_api_key.trim().is_empty(),
		PIXABAY => !config().pixabay_api_key.trim().is_empty(),
		LOC => config().loc_api_enabled,
		_ => false,
	}
}

/// Sources actuellement activées (clé présente pour les fournisseurs à clé).
pub fn enabled_sources() -> Vec<&'static str> {
	[NASA, BLENDER, EUROPEANA, PEXELS, PIXABAY, LOC]
		.into_iter()
		.filter(|source| enabled(source))
		.collect()
}

pub fn label_for_source(source: &str) -> &'static str {
	match source {
		NASA => "NASA Image & Video Library",
		BLENDER => "Blender Open Movies",
		EUROPEANA => "Europeana",
		PEXELS => "Pexels",
		PIXABAY => "Pixabay",
		LOC => "Library of Congress",
		_ => "Catalogue libre",
	}
}

pub fn is_open_id(value: &str) -> bool {
	source_of_id(value).is_some()
}

pub fn source_of_id(value: &str) -> Option<&'static str> {
	let (prefix, _) = value.split_once(':')?;
	match prefix {
		NASA => Some(NASA),
		BLENDER => Some(BLENDER),
		EUROPEANA => Some(EUROPEANA),
		PEXELS => Some(PEXELS),
		PIXABAY => Some(PIXABAY),
		LOC => Some(LOC),
		_ => None,
	}
}

pub fn label(source_id: &str) -> &'static str {
	match source_of_id(source_id) {
		Some(NASA) => "NASA Image & Video Library",
		Some(BLENDER) => "Blender Open Movies",
		Some(EUROPEANA) => "Europeana",
		Some(PEXELS) => "Pexels",
		Some(PIXABAY) => "Pixabay",
		Some(LOC) => "Library of Congress",
		_ => "Catalogue libre",
	}
}

pub fn sources() -> Vec<Value> {
	let mut list = vec![
		json!({
			"id": NASA,
			"label": "NASA Image & Video Library",
			"note": "Vidéos du domaine public de la NASA"
		}),
		json!({
			"id": BLENDER,
			"label": "Blender Open Movies",
			"note": "Films libres de la Blender Foundation"
		}),
	];
	if enabled(EUROPEANA) {
		list.push(json!({
			"id": EUROPEANA,
			"label": "Europeana",
			"note": "Patrimoine culturel européen (vidéos)"
		}));
	}
	if enabled(PEXELS) {
		list.push(json!({
			"id": PEXELS,
			"label": "Pexels",
			"note": "Vidéos de stock sous licence libre"
		}));
	}
	if enabled(PIXABAY) {
		list.push(json!({
			"id": PIXABAY,
			"label": "Pixabay",
			"note": "Vidéos de stock sous licence Pixabay"
		}));
	}
	if enabled(LOC) {
		list.push(json!({
			"id": LOC,
			"label": "Library of Congress",
			"note": "Films du domaine public américain"
		}));
	}
	list
}

pub fn collections(source: &str) -> Value {
	let pairs: &[(&str, &str)] = match source {
		NASA => &[
			("apollo", "Programme Apollo"),
			("artemis", "Artemis"),
			("space shuttle", "Navettes spatiales"),
			("earth", "La Terre"),
			("mars", "Mars"),
			("iss", "Station spatiale"),
		],
		BLENDER => &[
			("all", "Tous les films"),
			("2008", "Big Buck Bunny"),
			("2010", "Sintel"),
			("2006", "Elephants Dream"),
			("2012", "Tears of Steel"),
		],
		EUROPEANA => &[
			("film", "Films"),
			("documentary", "Documentaires"),
			("newsreel", "Actualités"),
			("animation", "Animation"),
		],
		PEXELS | PIXABAY => &[
			("nature", "Nature"),
			("city", "Villes"),
			("ocean", "Océan"),
			("space", "Espace"),
			("people", "Personnes"),
			("abstract", "Abstrait"),
		],
		LOC => &[
			("feature-films", "Feature films"),
			("national-film-registry", "National Film Registry"),
			("early-motion-pictures", "Early motion pictures"),
		],
		_ => &[("all", "Tous")],
	};
	json!(
		pairs
			.iter()
			.map(|(id, label)| json!({ "id": id, "label": label }))
			.collect::<Vec<_>>()
	)
}

pub fn default_collection(source: &str) -> &'static str {
	match source {
		NASA => "apollo",
		BLENDER => "all",
		EUROPEANA => "film",
		PEXELS | PIXABAY => "nature",
		LOC => "feature-films",
		_ => "all",
	}
}

pub fn licenses(source: &str) -> Value {
	let label = match source {
		NASA => "Domaine public NASA",
		BLENDER => "Creative Commons (Blender Foundation)",
		EUROPEANA => "Licences ouvertes / domaine public",
		PEXELS => "Licence Pexels",
		PIXABAY => "Licence Pixabay",
		LOC => "Domaine public / droits déclarés",
		_ => "Licence libre",
	};
	json!([{ "id": "free", "label": label }])
}

pub struct OpenSearchParams<'a> {
	pub source: &'a str,
	pub q: &'a str,
	pub collection: &'a str,
	pub page: i64,
	pub rows: i64,
	pub sort: Option<&'a str>,
}

pub async fn search(params: OpenSearchParams<'_>) -> Result<Arc<Value>> {
	let source = params.source;
	if !enabled(source) {
		return Err(ApiError::new(503, "Cette source n'est pas configurée"));
	}
	match source {
		NASA => search_nasa(&params).await,
		BLENDER => Ok(blender_search(&params)),
		PEXELS => search_pexels(&params).await,
		PIXABAY => search_pixabay(&params).await,
		EUROPEANA => search_europeana(&params).await,
		LOC => search_loc(&params).await,
		_ => Err(ApiError::bad_request("Source libre inconnue")),
	}
}

fn key_of(identifier: &str) -> Option<(&'static str, String)> {
	let source = source_of_id(identifier)?;
	let (_, encoded) = identifier.split_once(':')?;
	let key = hex_decode(encoded)?;
	Some((source, key))
}

fn json_id(source: &str, key: &str) -> String {
	format!("{source}:{}", hex_encode(key.as_bytes()))
}

fn hex_encode(bytes: &[u8]) -> String {
	bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(value: &str) -> Option<String> {
	if value.len() % 2 != 0 {
		return None;
	}
	let mut bytes = Vec::with_capacity(value.len() / 2);
	let mut index = 0;
	while index < value.len() {
		let byte = u8::from_str_radix(&value[index..index + 2], 16).ok()?;
		bytes.push(byte);
		index += 2;
	}
	String::from_utf8(bytes).ok()
}

fn file_name_of_url(url: &str) -> Option<String> {
	let clean = url.split(['?', '#']).next()?.trim();
	let name = clean.rsplit('/').next()?.trim();
	if name.is_empty() || name.len() > 180 || !name.contains('.') {
		return None;
	}
	let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
	Some(name)
}

fn encode_path(value: &str) -> String {
	value
		.split('/')
		.filter(|part| !part.is_empty())
		.map(urlencode)
		.collect::<Vec<_>>()
		.join("/")
}

fn quality_from_height(height: i64) -> String {
	match height {
		h if h >= 2000 => "4K".to_string(),
		h if h >= 1000 => "1080p".to_string(),
		h if h >= 700 => "720p".to_string(),
		h if h >= 450 => "480p".to_string(),
		h if h > 0 => "360p".to_string(),
		_ => "Auto".to_string(),
	}
}

fn format_of_name(name: &str) -> String {
	name.rsplit('.')
		.next()
		.map(|ext| ext.to_ascii_uppercase())
		.filter(|ext| !ext.is_empty())
		.unwrap_or_else(|| "Vidéo".to_string())
}

fn base_card(
	source: &str,
	key: &str,
	title: &str,
	overview: &str,
	poster: Option<&str>,
	year: Option<i64>,
	creator: Option<&str>,
	license: Option<&str>,
	duration: Option<i64>,
) -> Value {
	json!({
		"id": format!("{source}-{key}"),
		"mediaType": "archive",
		"archiveId": json_id(source, key),
		"sourceId": json_id(source, key),
		"source": source,
		"sourceLabel": label(&json_id(source, key)),
		"badge": "GRATUIT",
		"title": title,
		"overview": overview,
		"poster": poster,
		"thumb": poster,
		"thumbSmall": poster,
		"backdrop": poster,
		"backdropMobile": poster,
		"year": year,
		"rating": Value::Null,
		"downloads": Value::Null,
		"creator": creator,
		"license": license,
		"licenseLabel": licenses(source)
			.as_array()
			.and_then(|items| items.first())
			.and_then(|item| item.get("label"))
			.cloned()
			.unwrap_or(Value::Null),
		"collection": Value::Null,
		"language": Value::Null,
		"runtime": duration.map(|seconds| (seconds / 60).max(1)),
		"duration": duration,
		"genres": json!([]),
		"genreIds": json!([]),
	})
}

fn detail_from_card(mut card: Value, streams: Vec<Value>, details_url: Option<String>) -> Value {
	if let Some(object) = card.as_object_mut() {
		object.insert("streams".to_string(), Value::Array(streams));
		object.insert("episodes".to_string(), json!([]));
		object.insert("kind".to_string(), Value::String("movie".to_string()));
		object.insert("subjects".to_string(), json!([]));
		object.insert("embedUrl".to_string(), Value::Null);
		object.insert(
			"detailsUrl".to_string(),
			details_url.map(Value::String).unwrap_or(Value::Null),
		);
	}
	card
}

fn stream_value(name: &str, quality: &str, url: &str, size: u64) -> Value {
	json!({
		"name": name,
		"quality": quality,
		"format": format_of_name(name),
		"size": size,
		"url": url,
		"directUrl": url,
	})
}

// --- NASA ------------------------------------------------------------------

fn nasa_card(item: &Value) -> Option<Value> {
	let data = item.get("data")?.as_array()?.first()?;
	let nasa_id = data.get("nasa_id")?.as_str()?.trim();
	if nasa_id.is_empty() {
		return None;
	}
	let title = data
		.get("title")
		.and_then(Value::as_str)
		.unwrap_or(nasa_id)
		.trim();
	let description = strip_html(data.get("description"));
	let poster = item
		.get("links")
		.and_then(Value::as_array)
		.and_then(|links| {
			links.iter().find_map(|link| {
				let href = link.get("href")?.as_str()?;
				let render = link.get("render").and_then(Value::as_str).unwrap_or("");
				if render == "image" && href.to_ascii_lowercase().contains("~medium") {
					Some(href)
				} else {
					None
				}
			})
		})
		.or_else(|| {
			item.get("links")
				.and_then(Value::as_array)
				.and_then(|links| links.first())
				.and_then(|link| link.get("href"))
				.and_then(Value::as_str)
		});
	let year = data
		.get("date_created")
		.and_then(Value::as_str)
		.and_then(|date| date.get(0..4))
		.and_then(|value| value.parse::<i64>().ok());
	let creator = data.get("center").and_then(Value::as_str);

	Some(base_card(
		NASA,
		nasa_id,
		title,
		&description,
		poster,
		year,
		creator,
		Some("Domaine public NASA"),
		None,
	))
}

async fn search_nasa(params: &OpenSearchParams<'_>) -> Result<Arc<Value>> {
	let page = params.page.max(1);
	let rows = params.rows.clamp(1, 60);
	let term = if !params.q.trim().is_empty() {
		params.q.trim().to_string()
	} else {
		params.collection.trim().to_string()
	};
	let cache_key = format!("open:nasa:{term}:{page}:{rows}");
	let query = term.clone();
	cache()
		.cached(&cache_key, config().ttl.medium, move || async move {
			let url = build_url(
				&format!("{}/search", config().nasa_api_url),
				&[
					("q", query),
					("media_type", "video".to_string()),
					("page", page.to_string()),
					("page_size", rows.to_string()),
				],
			);
			let payload = fetch_json(&url, 15_000, 1, &[]).await?;
			let collection = payload.get("collection").cloned().unwrap_or(json!({}));
			let total = collection
				.pointer("/metadata/total_hits")
				.and_then(Value::as_i64)
				.unwrap_or(0);
			let items: Vec<Value> = collection
				.get("items")
				.and_then(Value::as_array)
				.map(|items| items.iter().filter_map(nasa_card).collect())
				.unwrap_or_default();
			Ok(Arc::new(json!({
				"page": page,
				"rows": rows,
				"total": total,
				"hasMore": i64::from(page) * rows < total,
				"items": items,
			})))
		})
		.await
}

async fn nasa_title(nasa_id: &str) -> Result<Value> {
	let url = build_url(
		&format!("{}/search", config().nasa_api_url),
		&[("nasa_id", nasa_id.to_string())],
	);
	let payload = fetch_json(&url, 15_000, 1, &[]).await?;
	let item = payload
		.pointer("/collection/items/0")
		.cloned()
		.ok_or_else(|| ApiError::not_found("Vidéo NASA introuvable"))?;
	let card = nasa_card(&item).ok_or_else(|| ApiError::not_found("Vidéo NASA introuvable"))?;

	let asset_url = format!(
		"{}/asset/{}",
		config().nasa_api_url,
		urlencode(nasa_id)
	);
	let asset = fetch_json(&asset_url, 15_000, 1, &[]).await?;
	let mut streams = Vec::new();
	if let Some(items) = asset.pointer("/collection/items").and_then(Value::as_array) {
		for item in items {
			let Some(href) = item.get("href").and_then(Value::as_str) else {
				continue;
			};
			if !VIDEO_EXT.is_match(href) {
				continue;
			}
			let Some(name) = file_name_of_url(href) else {
				continue;
			};
			let lower = name.to_ascii_lowercase();
			let quality = if lower.contains("orig") || lower.contains("4k") {
				"4K"
			} else if lower.contains("large") || lower.contains("1080") {
				"1080p"
			} else if lower.contains("medium") || lower.contains("720") {
				"720p"
			} else if lower.contains("small") || lower.contains("480") {
				"480p"
			} else {
				"Auto"
			};
			streams.push(stream_value(&name, quality, href, 0));
		}
	}
	streams.sort_by(|a, b| {
		let rank = |value: &Value| match value.get("quality").and_then(Value::as_str) {
			Some("4K") => 0,
			Some("1080p") => 1,
			Some("720p") => 2,
			Some("480p") => 3,
			_ => 4,
		};
		rank(a).cmp(&rank(b))
	});
	if streams.is_empty() {
		return Err(ApiError::new(422, "Aucune vidéo lisible pour ce média NASA"));
	}
	let details = Some(format!("https://images.nasa.gov/details/{}", urlencode(nasa_id)));
	Ok(detail_from_card(card, streams, details))
}

// --- Blender Open Movies ---------------------------------------------------

struct BlenderFilm {
	slug: &'static str,
	title: &'static str,
	overview: &'static str,
	year: i64,
	poster: Option<&'static str>,
	files: &'static [(&'static str, &'static str)],
}

const BLENDER_FILMS: &[BlenderFilm] = &[
	BlenderFilm {
		slug: "big-buck-bunny",
		title: "Big Buck Bunny",
		overview: "Court-métrage libre de la Blender Foundation (2008), réalisé par Sacha Goedegebure.",
		year: 2008,
		poster: Some("https://media.xiph.org/BBB/poster_big.jpg"),
		files: &[(
			"720p",
			"https://archive.org/download/BigBuckBunny_124/Content/big_buck_bunny_720p_surround.mp4",
		)],
	},
	BlenderFilm {
		slug: "sintel",
		title: "Sintel",
		overview: "Court-métrage libre de la Blender Foundation (2010), réalisé par Colin Levy.",
		year: 2010,
		poster: None,
		files: &[(
			"1080p",
			"https://archive.org/download/Sintel/sintel-2048-surround.mp4",
		)],
	},
	BlenderFilm {
		slug: "elephants-dream",
		title: "Elephants Dream",
		overview: "Premier film libre de la Blender Foundation (2006), réalisé par Bassam Kurdali.",
		year: 2006,
		poster: Some("https://media.xiph.org/ED/ED-poster.png"),
		files: &[(
			"720p",
			"https://archive.org/download/ElephantsDream/ed_1024_512kb.mp4",
		)],
	},
	BlenderFilm {
		slug: "tears-of-steel",
		title: "Tears of Steel",
		overview: "Court-métrage libre de la Blender Foundation (2012), réalisé par Ian Hubert.",
		year: 2012,
		poster: None,
		files: &[(
			"1080p",
			"https://media.xiph.org/tearsofsteel/tears_of_steel_1080p.webm",
		)],
	},
];

fn blender_card(film: &BlenderFilm) -> Value {
	base_card(
		BLENDER,
		film.slug,
		film.title,
		film.overview,
		film.poster,
		Some(film.year),
		Some("Blender Foundation"),
		Some("Creative Commons"),
		None,
	)
}

fn blender_search(params: &OpenSearchParams<'_>) -> Arc<Value> {
	let page = params.page.max(1);
	let rows = params.rows.clamp(1, 60);
	let term = params.q.trim().to_ascii_lowercase();
	let collection = params.collection.trim();
	let matches: Vec<&BlenderFilm> = BLENDER_FILMS
		.iter()
		.filter(|film| {
			let collection_ok = collection.is_empty()
				|| collection == "all"
				|| collection == film.year.to_string()
				|| collection == film.slug;
			let term_ok = term.is_empty()
				|| film.title.to_ascii_lowercase().contains(&term)
				|| film.overview.to_ascii_lowercase().contains(&term);
			collection_ok && term_ok
		})
		.collect();
	let total = matches.len() as i64;
	let start = ((page - 1) * rows).max(0) as usize;
	let items: Vec<Value> = matches
		.into_iter()
		.skip(start)
		.take(rows as usize)
		.map(blender_card)
		.collect();
	Arc::new(json!({
		"page": page,
		"rows": rows,
		"total": total,
		"hasMore": page * rows < total,
		"items": items,
	}))
}

fn blender_title(slug: &str) -> Result<Value> {
	let film = BLENDER_FILMS
		.iter()
		.find(|film| film.slug == slug)
		.ok_or_else(|| ApiError::not_found("Film Blender introuvable"))?;
	let streams = film
		.files
		.iter()
		.filter_map(|(quality, url)| {
			let name = file_name_of_url(url)?;
			Some(stream_value(&name, quality, url, 0))
		})
		.collect();
	Ok(detail_from_card(
		blender_card(film),
		streams,
		Some("https://www.blender.org/about/projects/".to_string()),
	))
}

// --- Pexels ----------------------------------------------------------------

fn pexels_card(video: &Value) -> Option<Value> {
	let id = video.get("id").and_then(Value::as_i64)?;
	let key = id.to_string();
	let page_url = video.get("url").and_then(Value::as_str).unwrap_or("");
	let slug = page_url
		.trim_end_matches('/')
		.rsplit('/')
		.next()
		.unwrap_or("")
		.rsplit_once('-')
		.map(|(title, _)| title.replace('-', " "))
		.unwrap_or_else(|| format!("Vidéo Pexels {id}"));
	let title = {
		let mut chars = slug.chars();
		match chars.next() {
			Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
			None => format!("Vidéo Pexels {id}"),
		}
	};
	let poster = video.get("image").and_then(Value::as_str);
	let duration = video.get("duration").and_then(Value::as_i64);
	let creator = video.pointer("/user/name").and_then(Value::as_str);
	Some(base_card(
		PEXELS,
		&key,
		&title,
		"Vidéo de stock Pexels, utilisable selon la licence Pexels.",
		poster,
		None,
		creator,
		Some("Licence Pexels"),
		duration,
	))
}

fn pexels_streams(video: &Value) -> Vec<Value> {
	let mut streams: Vec<Value> = video
		.get("video_files")
		.and_then(Value::as_array)
		.map(|files| {
			files
				.iter()
				.filter_map(|file| {
					let url = file.get("link").and_then(Value::as_str)?;
					if !VIDEO_EXT.is_match(url) {
						return None;
					}
					let name = file_name_of_url(url)?;
					let height = file.get("height").and_then(Value::as_i64).unwrap_or(0);
					let size = file
						.get("size")
						.and_then(|value| value.as_u64().or_else(|| value.as_f64().map(|v| v as u64)))
						.unwrap_or(0);
					Some(stream_value(&name, &quality_from_height(height), url, size))
				})
				.collect()
		})
		.unwrap_or_default();
	streams.sort_by(|a, b| {
		let height = |value: &Value| match value.get("quality").and_then(Value::as_str) {
			Some("4K") => 0,
			Some("1080p") => 1,
			Some("720p") => 2,
			Some("480p") => 3,
			_ => 4,
		};
		height(a).cmp(&height(b))
	});
	streams
}

async fn search_pexels(params: &OpenSearchParams<'_>) -> Result<Arc<Value>> {
	let page = params.page.max(1);
	let rows = params.rows.clamp(1, 60);
	let term = if !params.q.trim().is_empty() {
		params.q.trim().to_string()
	} else {
		params.collection.trim().to_string()
	};
	let key = config().pexels_api_key.clone();
	let cache_key = format!("open:pexels:{term}:{page}:{rows}");
	cache()
		.cached(&cache_key, config().ttl.medium, move || async move {
			let url = build_url(
				"https://api.pexels.com/videos/search",
				&[
					("query", term),
					("page", page.to_string()),
					("per_page", rows.to_string()),
				],
			);
			let payload = fetch_json(&url, 15_000, 1, &[("authorization", &key)]).await?;
			let total = payload
				.get("total_results")
				.and_then(Value::as_i64)
				.unwrap_or(0);
			let items: Vec<Value> = payload
				.get("videos")
				.and_then(Value::as_array)
				.map(|videos| videos.iter().filter_map(pexels_card).collect())
				.unwrap_or_default();
			Ok(Arc::new(json!({
				"page": page,
				"rows": rows,
				"total": total,
				"hasMore": page * rows < total,
				"items": items,
			})))
		})
		.await
}

async fn pexels_title(key: &str) -> Result<Value> {
	let url = format!("https://api.pexels.com/videos/videos/{}", urlencode(key));
	let key_header = config().pexels_api_key.clone();
	let payload = fetch_json(&url, 15_000, 1, &[("authorization", &key_header)]).await?;
	let card = pexels_card(&payload).ok_or_else(|| ApiError::not_found("Vidéo Pexels introuvable"))?;
	let streams = pexels_streams(&payload);
	if streams.is_empty() {
		return Err(ApiError::new(422, "Aucun fichier vidéo Pexels lisible"));
	}
	let details = payload.get("url").and_then(Value::as_str).map(str::to_string);
	Ok(detail_from_card(card, streams, details))
}

// --- Pixabay ---------------------------------------------------------------

fn pixabay_card(hit: &Value) -> Option<Value> {
	let id = hit.get("id").and_then(Value::as_i64)?;
	let tags = hit.get("tags").and_then(Value::as_str).unwrap_or("");
	let title = if tags.trim().is_empty() {
		format!("Vidéo Pixabay {id}")
	} else {
		tags.split(',')
			.next()
			.unwrap_or(tags)
			.trim()
			.to_string()
	};
	let poster = hit
		.pointer("/videos/large/thumbnail")
		.or_else(|| hit.pointer("/videos/medium/thumbnail"))
		.and_then(Value::as_str);
	let duration = hit.get("duration").and_then(Value::as_i64);
	let creator = hit.get("user").and_then(Value::as_str);
	let mut card = base_card(
		PIXABAY,
		&id.to_string(),
		&title,
		tags,
		poster,
		None,
		creator,
		Some("Licence Pixabay"),
		duration,
	);
	if let Some(object) = card.as_object_mut() {
		object.insert(
			"detailsUrl".to_string(),
			hit.get("pageURL")
				.cloned()
				.unwrap_or(Value::Null),
		);
	}
	Some(card)
}

fn pixabay_streams(hit: &Value) -> Vec<Value> {
	let mut streams = Vec::new();
	if let Some(videos) = hit.get("videos").and_then(Value::as_object) {
		for (variant, video) in videos {
			let Some(url) = video.get("url").and_then(Value::as_str) else {
				continue;
			};
			if !VIDEO_EXT.is_match(url) {
				continue;
			}
			let Some(name) = file_name_of_url(url) else {
				continue;
			};
			let quality = match variant.as_str() {
				"large" => "1080p",
				"medium" => "720p",
				"small" => "480p",
				"tiny" => "360p",
				_ => "Auto",
			};
			let size = video.get("size").and_then(Value::as_u64).unwrap_or(0);
			streams.push(stream_value(&name, quality, url, size));
		}
	}
	streams.sort_by(|a, b| {
		let rank = |value: &Value| match value.get("quality").and_then(Value::as_str) {
			Some("1080p") => 0,
			Some("720p") => 1,
			Some("480p") => 2,
			Some("360p") => 3,
			_ => 4,
		};
		rank(a).cmp(&rank(b))
	});
	streams
}

async fn search_pixabay(params: &OpenSearchParams<'_>) -> Result<Arc<Value>> {
	let page = params.page.max(1);
	let rows = params.rows.clamp(3, 60);
	let term = if !params.q.trim().is_empty() {
		params.q.trim().to_string()
	} else {
		params.collection.trim().to_string()
	};
	let key = config().pixabay_api_key.clone();
	let cache_key = format!("open:pixabay:{term}:{page}:{rows}");
	cache()
		.cached(&cache_key, config().ttl.medium, move || async move {
			let url = build_url(
				"https://pixabay.com/api/videos/",
				&[
					("key", key),
					("q", term),
					("page", page.to_string()),
					("per_page", rows.to_string()),
					("safesearch", "true".to_string()),
				],
			);
			let payload = fetch_json(&url, 15_000, 1, &[]).await?;
			let total = payload
				.get("totalHits")
				.and_then(Value::as_i64)
				.unwrap_or(0);
			let items: Vec<Value> = payload
				.get("hits")
				.and_then(Value::as_array)
				.map(|hits| hits.iter().filter_map(pixabay_card).collect())
				.unwrap_or_default();
			Ok(Arc::new(json!({
				"page": page,
				"rows": rows,
				"total": total,
				"hasMore": page * rows < total,
				"items": items,
			})))
		})
		.await
}

async fn pixabay_title(key: &str) -> Result<Value> {
	let api_key = config().pixabay_api_key.clone();
	let url = build_url(
		"https://pixabay.com/api/videos/",
		&[
			("key", api_key),
			("id", key.to_string()),
			("per_page", "3".to_string()),
		],
	);
	let payload = fetch_json(&url, 15_000, 1, &[]).await?;
	let hit = payload
		.get("hits")
		.and_then(Value::as_array)
		.and_then(|hits| hits.first())
		.ok_or_else(|| ApiError::not_found("Vidéo Pixabay introuvable"))?;
	let card = pixabay_card(hit).ok_or_else(|| ApiError::not_found("Vidéo Pixabay introuvable"))?;
	let streams = pixabay_streams(hit);
	if streams.is_empty() {
		return Err(ApiError::new(422, "Aucun fichier vidéo Pixabay lisible"));
	}
	let details = hit.get("pageURL").and_then(Value::as_str).map(str::to_string);
	Ok(detail_from_card(card, streams, details))
}

// --- Europeana -------------------------------------------------------------

fn europeana_card(item: &Value) -> Option<Value> {
	let id = item.get("id").and_then(Value::as_str)?;
	let title = first_value_str(item.get("title")).unwrap_or_else(|| format!("Europeana {id}"));
	let overview = strip_html(item.get("dcDescription"));
	let poster = first_value(item.get("edmPreview"))
		.and_then(Value::as_str)
		.map(str::to_string);
	let year = first_value_str(item.get("year"))
		.and_then(|value| value.get(0..4).and_then(|part| part.parse::<i64>().ok()));
	let creator = first_value_str(item.get("dcCreator"));
	let license = first_value_str(item.get("rights"));
	Some(base_card(
		EUROPEANA,
		id,
		&title,
		&overview,
		poster.as_deref(),
		year,
		creator.as_deref(),
		license.as_deref(),
		None,
	))
}

async fn search_europeana(params: &OpenSearchParams<'_>) -> Result<Arc<Value>> {
	let page = params.page.max(1);
	let rows = params.rows.clamp(1, 60);
	let term = if !params.q.trim().is_empty() {
		params.q.trim().to_string()
	} else {
		params.collection.trim().to_string()
	};
	let key = config().europeana_api_key.clone();
	let cache_key = format!("open:europeana:{term}:{page}:{rows}");
	cache()
		.cached(&cache_key, config().ttl.medium, move || async move {
			let url = build_url(
				&format!("{}/record/v2/search.json", config().europeana_api_url),
				&[
					("wskey", key),
					("query", term),
					("qf", "TYPE:VIDEO".to_string()),
					("rows", rows.to_string()),
					("start", ((page - 1) * rows + 1).to_string()),
				],
			);
			let payload = fetch_json(&url, 15_000, 1, &[]).await?;
			let total = payload
				.get("totalResults")
				.and_then(Value::as_i64)
				.unwrap_or(0);
			let items: Vec<Value> = payload
				.get("items")
				.and_then(Value::as_array)
				.map(|items| items.iter().filter_map(europeana_card).collect())
				.unwrap_or_default();
			Ok(Arc::new(json!({
				"page": page,
				"rows": rows,
				"total": total,
				"hasMore": page * rows < total,
				"items": items,
			})))
		})
		.await
}

fn collect_video_urls(value: &Value, out: &mut Vec<String>) {
	match value {
		Value::String(url) => {
			if url.starts_with("http") && VIDEO_EXT.is_match(url) && !out.iter().any(|item| item == url) {
				out.push(url.clone());
			}
		}
		Value::Array(items) => {
			for item in items {
				collect_video_urls(item, out);
			}
		}
		Value::Object(map) => {
			for item in map.values() {
				collect_video_urls(item, out);
			}
		}
		_ => {}
	}
}

async fn europeana_title(id: &str) -> Result<Value> {
	let key = config().europeana_api_key.clone();
	let url = build_url(
		&format!(
			"{}/record/v2/{}.json",
			config().europeana_api_url,
			encode_path(id)
		),
		&[("wskey", key)],
	);
	let payload = fetch_json(&url, 15_000, 1, &[]).await?;
	let mut object = payload.get("object").cloned().unwrap_or(json!({}));
	if let Some(map) = object.as_object_mut() {
		map.entry("id").or_insert_with(|| Value::String(id.to_string()));
	}
	let card = europeana_card(&object)
		.ok_or_else(|| ApiError::not_found("Média Europeana introuvable"))?;
	let mut urls = Vec::new();
	collect_video_urls(&object, &mut urls);
	let streams: Vec<Value> = urls
		.iter()
		.filter_map(|url| {
			let name = file_name_of_url(url)?;
			Some(stream_value(&name, "Auto", url, 0))
		})
		.collect();
	if streams.is_empty() {
		return Err(ApiError::new(422, "Aucune vidéo lisible pour ce média Europeana"));
	}
	let details = Some(format!(
		"https://www.europeana.eu/fr/item/{}",
		urlencode(id)
	));
	Ok(detail_from_card(card, streams, details))
}

// --- Library of Congress ---------------------------------------------------

fn loc_card(result: &Value) -> Option<Value> {
	let id = result.get("id").and_then(Value::as_str)?;
	let title = first_value_str(result.get("title")).unwrap_or_else(|| format!("Library of Congress {id}"));
	let overview = strip_html(result.get("description"));
	let poster = first_value(result.get("image_url"))
		.and_then(Value::as_str)
		.map(str::to_string);
	let year = first_value_str(result.get("date"))
		.and_then(|value| value.get(0..4).and_then(|part| part.parse::<i64>().ok()));
	let creator = first_value_str(result.get("contributor"));
	let license = first_value_str(result.get("rights"));
	Some(base_card(
		LOC,
		id,
		&title,
		&overview,
		poster.as_deref(),
		year,
		creator.as_deref(),
		license.as_deref(),
		None,
	))
}

async fn search_loc(params: &OpenSearchParams<'_>) -> Result<Arc<Value>> {
	let page = params.page.max(1);
	let rows = params.rows.clamp(1, 60);
	let collection = if params.collection.trim().is_empty() {
		default_collection(LOC)
	} else {
		params.collection.trim()
	};
	let collection = collection.to_string();
	let term = params.q.trim().to_string();
	let cache_key = format!("open:loc:{collection}:{term}:{page}:{rows}");
	cache()
		.cached(&cache_key, config().ttl.medium, move || async move {
			let mut pairs = vec![
				("fo", "json".to_string()),
				("c", rows.to_string()),
				("sp", page.to_string()),
			];
			if !term.is_empty() {
				pairs.push(("q", term));
			}
			let url = build_url(
				&format!("{}/collections/{}/", config().loc_api_url, urlencode(&collection)),
				&pairs,
			);
			let payload = fetch_json(&url, 15_000, 1, &[]).await?;
			let total = payload
				.get("pagination")
				.and_then(|value| value.get("of"))
				.and_then(Value::as_i64)
				.unwrap_or(0);
			let items: Vec<Value> = payload
				.get("results")
				.and_then(Value::as_array)
				.map(|results| results.iter().filter_map(loc_card).collect())
				.unwrap_or_default();
			Ok(Arc::new(json!({
				"page": page,
				"rows": rows,
				"total": total,
				"hasMore": page * rows < total,
				"items": items,
			})))
		})
		.await
}

async fn loc_title(id: &str) -> Result<Value> {
	let url = build_url(
		&format!("{}/item/{}/", config().loc_api_url, urlencode(id)),
		&[("fo", "json".to_string())],
	);
	let payload = fetch_json(&url, 15_000, 1, &[]).await?;
	let card = loc_card(&payload)
		.or_else(|| payload.get("item").and_then(loc_card))
		.ok_or_else(|| ApiError::not_found("Média Library of Congress introuvable"))?;
	let mut urls = Vec::new();
	collect_video_urls(&payload, &mut urls);
	let streams: Vec<Value> = urls
		.iter()
		.filter_map(|url| {
			let name = file_name_of_url(url)?;
			Some(stream_value(&name, "Auto", url, 0))
		})
		.collect();
	if streams.is_empty() {
		return Err(ApiError::new(
			422,
			"Aucune vidéo lisible pour ce média Library of Congress",
		));
	}
	let details = Some(format!("{}/item/{}/", config().loc_api_url, id));
	Ok(detail_from_card(card, streams, details))
}

// --- Entrées publiques -----------------------------------------------------

pub async fn get_title(identifier: &str) -> Result<Value> {
	let Some((source, key)) = key_of(identifier) else {
		return Err(ApiError::bad_request("Identifiant de source libre invalide"));
	};
	match source {
		NASA => nasa_title(&key).await,
		BLENDER => blender_title(&key),
		PEXELS => pexels_title(&key).await,
		PIXABAY => pixabay_title(&key).await,
		EUROPEANA => europeana_title(&key).await,
		LOC => loc_title(&key).await,
		_ => Err(ApiError::bad_request("Source libre inconnue")),
	}
}

pub async fn get_file(source_id: &str, file_name: &str) -> Result<Value> {
	let title = get_title(source_id).await?;
	let stream = title
		.get("streams")
		.and_then(Value::as_array)
		.and_then(|streams| {
			streams
				.iter()
				.find(|stream| stream.get("name").and_then(Value::as_str) == Some(file_name))
		})
		.ok_or_else(|| ApiError::not_found("Ce fichier n'existe plus pour cette source"))?;
	let url = stream
		.get("url")
		.and_then(Value::as_str)
		.ok_or_else(|| ApiError::not_found("URL du fichier introuvable"))?;
	Ok(json!({
		"identifier": source_id,
		"name": file_name,
		"size": stream.get("size").cloned().unwrap_or(Value::from(0)),
		"format": stream.get("format").cloned().unwrap_or(Value::from("Vidéo")),
		"url": url,
	}))
}
