//! Catalogue mondial TMDB : client authentifié + cartes normalisées pour le front.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::cache::cache;
use crate::config::{config, has_tmdb_key};
use crate::error::{ApiError, Result};
use crate::http::{build_url, fetch_json};

pub const SUPPORTED_LANGS: [&str; 5] = ["fr-FR", "en-US", "es-ES", "pt-BR", "ar-SA"];
pub const DEFAULT_LANG: &str = "fr-FR";
pub const MEDIA_TYPES: [&str; 2] = ["movie", "tv"];

pub fn resolve_lang(value: Option<&str>) -> String {
	match value {
		Some(value) if SUPPORTED_LANGS.contains(&value) => value.to_string(),
		_ => DEFAULT_LANG.to_string(),
	}
}

pub fn is_media_type(value: &str) -> bool {
	MEDIA_TYPES.contains(&value)
}

/// Appel TMDB authentifié + cache mémoire (clé = URL complète).
pub async fn tmdb_request(
	path: &str,
	params: &[(&str, String)],
	ttl_ms: u64,
	lang: &str,
) -> Result<Arc<Value>> {
	if !has_tmdb_key() {
		return Err(ApiError::new(
			503,
			"Cle TMDB absente : ajoute TMDB_API_KEY (ou VITE_API_KEY) dans le fichier .env",
		));
	}

	let mut all_params: Vec<(&str, String)> = vec![("api_key", config().tmdb_api_key.clone())];
	if !params.iter().any(|(key, _)| *key == "language") {
		all_params.push(("language", resolve_lang(Some(lang))));
	}
	all_params.extend_from_slice(params);

	let url = build_url(&format!("{}{}", config().tmdb_base_url, path), &all_params);
	let key = format!("tmdb:{url}");

	cache()
		.cached(&key, ttl_ms, || async {
			let value = fetch_json(&url, 12_000, 1, &[]).await?;
			Ok(Arc::new(value))
		})
		.await
}

pub fn image_url(path: Option<&str>, size: &str) -> Option<String> {
	path.map(|path| format!("{}/{}{}", config().tmdb_image_base_url, size, path))
}

fn year_from(value: Option<&str>) -> Option<i64> {
	let value = value?;
	let slice = value.get(..4)?;
	let year: i64 = slice.parse().ok()?;
	if year > 1800 {
		Some(year)
	} else {
		None
	}
}

pub fn round1(value: f64) -> f64 {
	(value * 10.0).round() / 10.0
}

/// Transforme une entrée TMDB (film ou série) en « carte » normalisée pour le front.
pub fn card_from_tmdb(item: &Value, fallback_type: &str) -> Option<Value> {
	if item.is_null() {
		return None;
	}

	let media_type_raw = item.get("media_type").and_then(|value| value.as_str());
	let media_type = match media_type_raw {
		Some("person") => return None,
		Some(value) if is_media_type(value) => value.to_string(),
		_ => fallback_type.to_string(),
	};

	let title = item
		.get("title")
		.and_then(|value| value.as_str())
		.or_else(|| item.get("name").and_then(|value| value.as_str()))
		.or_else(|| item.get("original_title").and_then(|value| value.as_str()))
		.or_else(|| item.get("original_name").and_then(|value| value.as_str()))?;
	let id = item.get("id")?;

	let date = if media_type == "tv" {
		item.get("first_air_date")
	} else {
		item.get("release_date")
	}
	.and_then(|value| value.as_str());

	let poster_path = item.get("poster_path").and_then(|value| value.as_str());
	let backdrop_path = item.get("backdrop_path").and_then(|value| value.as_str());

	let rating = item
		.get("vote_average")
		.and_then(|value| value.as_f64())
		.map(round1);

	let genre_ids = match item.get("genre_ids") {
		Some(value) if value.is_array() => value.clone(),
		_ => match item.get("genres") {
			Some(Value::Array(genres)) => Value::Array(
				genres
					.iter()
					.filter_map(|genre| genre.get("id").cloned())
					.collect(),
			),
			_ => json!([]),
		},
	};

	let genres = match item.get("genres") {
		Some(Value::Array(genres)) => genres
			.iter()
			.filter_map(|genre| genre.get("name").and_then(|value| value.as_str()))
			.map(|value| Value::String(value.to_string()))
			.collect::<Vec<_>>(),
		_ => vec![],
	};

	Some(json!({
		"id": format!("{}-{}", media_type, id),
		"mediaType": media_type,
		"tmdbId": id,
		"source": "tmdb",
		"badge": if media_type == "tv" { "SÉRIE" } else { "FILM" },
		"title": title,
		"originalTitle": item
			.get("original_title")
			.and_then(|v| v.as_str())
			.or_else(|| item.get("original_name").and_then(|v| v.as_str()))
			.map(Value::from)
			.unwrap_or(Value::Null),
		"overview": item.get("overview").and_then(|v| v.as_str()).unwrap_or(""),
		"poster": image_url(poster_path, "w500").map(Value::String).unwrap_or(Value::Null),
		"thumb": image_url(poster_path, "w342").map(Value::String).unwrap_or(Value::Null),
		"thumbSmall": image_url(poster_path, "w185").map(Value::String).unwrap_or(Value::Null),
		"backdrop": image_url(backdrop_path, "w1280").map(Value::String).unwrap_or(Value::Null),
		"backdropMobile": image_url(backdrop_path, "w780").map(Value::String).unwrap_or(Value::Null),
		"year": year_from(date).map(Value::from).unwrap_or(Value::Null),
		"releaseDate": date.map(Value::from).unwrap_or(Value::Null),
		"rating": rating.map(Value::from).unwrap_or(Value::Null),
		"votes": item.get("vote_count").cloned().unwrap_or(Value::Null),
		"popularity": item.get("popularity").cloned().unwrap_or(json!(0)),
		"genreIds": genre_ids,
		"genres": Value::Array(genres),
		"runtime": item.get("runtime").cloned().unwrap_or(Value::Null),
		"seasons": item.get("number_of_seasons").cloned().unwrap_or(Value::Null),
		"episodes": item.get("number_of_episodes").cloned().unwrap_or(Value::Null),
		"originalLanguage": item.get("original_language").cloned().unwrap_or(Value::Null),
	}))
}

pub fn cards_from_response(response: &Value, fallback_type: &str) -> Vec<Value> {
	response
		.get("results")
		.and_then(|value| value.as_array())
		.map(|items| {
			items
				.iter()
				.filter_map(|item| card_from_tmdb(item, fallback_type))
				.collect()
		})
		.unwrap_or_default()
}

pub fn find_trailer(videos: &Value) -> Option<Value> {
	let list = videos.as_array()?;
	let youtube: Vec<&Value> = list
		.iter()
		.filter(|video| {
			video.get("site").and_then(|value| value.as_str()) == Some("YouTube")
				&& video.get("key").and_then(|value| value.as_str()).is_some()
		})
		.collect();

	let by_type = |kind: &str| {
		youtube
			.iter()
			.find(|video| video.get("type").and_then(|value| value.as_str()) == Some(kind))
	};

	let trailer = by_type("Trailer")
		.or_else(|| by_type("Teaser"))
		.or_else(|| youtube.first())?;

	let key = trailer.get("key")?.as_str()?;
	let name = trailer
		.get("name")
		.and_then(|value| value.as_str())
		.unwrap_or("Bande-annonce");

	Some(json!({
		"key": key,
		"name": name,
		"embedUrl": format!("https://www.youtube.com/embed/{key}?autoplay=1&rel=0"),
		"watchUrl": format!("https://www.youtube.com/watch?v={key}"),
	}))
}

/// Liste des plateformes légales de diffusion, avec un drapeau « gratuit ».
pub fn providers_from_response(response: &Value, region: &str) -> Value {
	let region_data = response
		.get("results")
		.and_then(|results| results.get(region))
		.or_else(|| response.get("results").and_then(|results| results.get("US")));

	let Some(region_data) = region_data else {
		return json!({ "region": region, "link": null, "groups": [] });
	};

	let groups_defs = [
		("free", "Gratuit"),
		("ads", "Gratuit avec publicite"),
		("flatrate", "Inclus dans un abonnement"),
		("rent", "Location"),
		("buy", "Achat"),
	];

	let mut groups = Vec::new();
	for (key, label) in groups_defs {
		let is_free = key == "free" || key == "ads";
		let items: Vec<Value> = region_data
			.get(key)
			.and_then(|value| value.as_array())
			.map(|providers| {
				providers
					.iter()
					.map(|provider| {
						json!({
							"id": provider.get("provider_id").cloned().unwrap_or(Value::Null),
							"name": provider.get("provider_name").cloned().unwrap_or(Value::Null),
							"logo": image_url(
								provider.get("logo_path").and_then(|value| value.as_str()),
								"w92",
							)
							.map(Value::String)
							.unwrap_or(Value::Null),
							"free": is_free,
						})
					})
					.collect()
			})
			.unwrap_or_default();

		if !items.is_empty() {
			groups.push(json!({ "label": label, "free": is_free, "items": items }));
		}
	}

	json!({
		"region": region,
		"link": region_data.get("link").cloned().unwrap_or(Value::Null),
		"groups": groups,
	})
}

pub fn credits_from_response(credits: &Value) -> Value {
	let crew = credits
		.get("crew")
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default();
	let cast: Vec<Value> = credits
		.get("cast")
		.and_then(|value| value.as_array())
		.map(|items| {
			items
				.iter()
				.take(16)
				.map(|person| {
					json!({
						"id": person.get("id").cloned().unwrap_or(Value::Null),
						"name": person.get("name").cloned().unwrap_or(Value::Null),
						"character": person.get("character").cloned().unwrap_or(Value::Null),
						"photo": image_url(
							person.get("profile_path").and_then(|value| value.as_str()),
							"w185",
						)
						.map(Value::String)
						.unwrap_or(Value::Null),
					})
				})
				.collect()
		})
		.unwrap_or_default();

	let directors: Vec<Value> = crew
		.iter()
		.filter(|member| member.get("job").and_then(|value| value.as_str()) == Some("Director"))
		.filter_map(|member| member.get("name").cloned())
		.collect();

	let mut writers: Vec<String> = Vec::new();
	for member in &crew {
		let job = member.get("job").and_then(|value| value.as_str()).unwrap_or("");
		if ["Writer", "Screenplay", "Story"].contains(&job) {
			if let Some(name) = member.get("name").and_then(|value| value.as_str()) {
				if !writers.iter().any(|existing| existing == name) {
					writers.push(name.to_string());
				}
			}
		}
	}
	writers.truncate(4);
	let writers: Vec<Value> = writers.into_iter().map(Value::String).collect();

	json!({ "cast": cast, "directors": directors, "writers": writers })
}
