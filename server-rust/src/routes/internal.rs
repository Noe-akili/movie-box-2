//! Routes de la source interne (`/api/internal/*`) + proxy média local.

use axum::extract::{Path, Query, Request};
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::{ApiError, Result};
use crate::services::internal;

pub fn router() -> Router {
	Router::new()
		.route("/health", get(health))
		.route("/search", get(search))
		.route("/title/{id}", get(title))
		.route("/stream/{id}", get(stream))
		.route("/episodes/{id}", get(episodes))
		.route("/proxy", get(proxy))
}

async fn health() -> Json<Value> {
	Json(internal::health().await)
}

async fn search(Query(params): Query<HashMap<String, String>>) -> Result<Json<Value>> {
	let query = params.get("q").cloned().unwrap_or_default();
	let page = params
		.get("page")
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(1)
		.max(1);
	let rows = params
		.get("rows")
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(40)
		.clamp(1, 60);
	let collection = params
		.get("collection")
		.cloned()
		.unwrap_or_else(|| internal::DEFAULT_COLLECTION.to_string());

	let result = internal::search(&query, &collection, page, rows).await?;
	let count = result
		.get("items")
		.and_then(|value| value.as_array())
		.map(|items| items.len())
		.unwrap_or(0);

	Ok(Json(json!({
		"query": query,
		"page": page,
		"count": count,
		"items": result.get("items").cloned().unwrap_or(json!([])),
	})))
}

async fn title(Path(id): Path<String>) -> Result<Json<Value>> {
	Ok(Json(internal::title(&id).await?))
}

async fn stream(Path(id): Path<String>, Query(params): Query<HashMap<String, String>>) -> Result<Json<Value>> {
	let season = params
		.get("season")
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(1);
	let episode = params
		.get("episode")
		.and_then(|value| value.parse::<i64>().ok())
		.unwrap_or(1);
	let quality = params
		.get("quality")
		.cloned()
		.unwrap_or_else(|| "720p".to_string());

	let streams = internal::streams(&id, season, episode, &quality).await?;
	Ok(Json(json!({
		"id": id,
		"season": season,
		"episode": episode,
		"streams": streams,
	})))
}

async fn episodes(Path(id): Path<String>) -> Result<Json<Value>> {
	let items = internal::episodes(&id).await?;
	Ok(Json(json!({ "id": id, "episodes": items })))
}

fn is_blocked_host(host: &str) -> bool {
	let host = host.to_ascii_lowercase();
	let host = host.split('%').next().unwrap_or(&host);

	if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
		return true;
	}
	if host.starts_with("127.") || host == "0.0.0.0" || host == "::1" || host == "[::1]" {
		return true;
	}
	if host.starts_with("10.") || host.starts_with("192.168.") || host.starts_with("169.254.") {
		return true;
	}
	if let Some(second) = host.split('.').nth(1) {
		if host.starts_with("172.")
			&& matches!(second.parse::<u16>(), Ok(value) if (16..=31).contains(&value))
		{
			return true;
		}
	}
	false
}

/// Type MIME déduit de l'extension du flux (l'URL peut contenir une signature).
fn stream_mime(src: &str) -> Option<&'static str> {
	let path = src.split(['?', '#']).next().unwrap_or(src).to_ascii_lowercase();
	let path = path.split('/').next_back().unwrap_or(&path);

	if path.ends_with(".mp4") || path.ends_with(".m4v") {
		Some("video/mp4")
	} else if path.ends_with(".webm") {
		Some("video/webm")
	} else if path.ends_with(".ogv") || path.ends_with(".ogg") {
		Some("video/ogg")
	} else if path.ends_with(".mkv") {
		Some("video/x-matroska")
	} else if path.ends_with(".m3u8") {
		Some("application/vnd.apple.mpegurl")
	} else if path.ends_with(".mov") {
		Some("video/quicktime")
	} else if path.ends_with(".avi") {
		Some("video/x-msvideo")
	} else {
		None
	}
}

/// Proxy média : relaie un flux CDN vers le navigateur (CORS + en-tête `Cookie` de signature).
pub async fn proxy(request: Request) -> Result<Response> {
	let uri = request.uri().clone();
	let params: HashMap<String, String> = uri
		.query()
		.and_then(|query|
			serde_urlencoded::from_str(query).ok())
		.unwrap_or_default();

	let src = params
		.get("src")
		.cloned()
		.ok_or_else(|| ApiError::bad_request("Paramètre « src » requis"))?;
	let cookie = params.get("cookie").cloned().unwrap_or_default();

	let url = reqwest::Url::parse(&src)
		.map_err(|_| ApiError::bad_request("URL de flux invalide"))?;
	if url.scheme() != "http" && url.scheme() != "https" {
		return Err(ApiError::bad_request("Seuls les flux http(s) sont acceptés"));
	}
	let host = url.host_str().unwrap_or("").to_string();
	if is_blocked_host(&host) {
		return Err(ApiError::bad_request("Hôte de flux non autorisé"));
	}

	let range = request
		.headers()
		.get(header::RANGE)
		.and_then(|value| value.to_str().ok())
		.map(str::to_string);

	// Le CDN video (bcdnxw.hakunaymatata.com) applique une protection
	// anti-hotlink qui repond 429 sauf pour certains referers autorises.
	// `movieboxapp.in` est l'un d'eux : sans cet en-tete tous les flux
	// videos sont refuses.
	//
	// On rejoue la requete avec plusieurs profils car les flux changent de
	// CDN : le telechargeur du backend (UA `movie-box/1.0`, sans cookie) est
	// notre reference qui fonctionne. Un cookie de signature perime fait
	// echouer certains CDN, d'ou la tentative sans cookie en premier.
	let mut profiles: Vec<(&str, Option<&str>)> = vec![
		("movie-box/1.0", None),
		(
			"Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Mobile Safari/537.36",
			None,
		),
	];
	if !cookie.is_empty() {
		profiles.push(("movie-box/1.0", Some(cookie.as_str())));
	}

	let mut response = None;
	let mut last_status_response = None;
	for (agent, cookie_value) in profiles {
		let mut upstream = crate::http::client()
			.get(url.clone())
			.header("user-agent", agent)
			.header("accept", "*/*")
			// Evite une decompression transparente : `content-length` doit rester
			// coherent avec le corps transmis pour que le seek fonctionne.
			.header("accept-encoding", "identity")
			.header("referer", "https://movieboxapp.in/")
			.header("origin", "https://h5.aoneroom.com");
		if let Some(value) = cookie_value {
			upstream = upstream.header("cookie", value);
		}
		if let Some(value) = &range {
			upstream = upstream.header("range", value);
		}

		match upstream.send().await {
			Ok(candidate)
				if candidate.status().is_success()
					|| candidate.status() == StatusCode::PARTIAL_CONTENT =>
			{
				response = Some(candidate);
				break;
			}
			Ok(candidate) => last_status_response = Some(candidate),
			Err(_) => continue,
		}
	}

	let response = response
		.or(last_status_response)
		.ok_or_else(|| ApiError::upstream(504, "Flux distant injoignable"))?;

	let status = response.status();
	if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
		eprintln!("[movie-box] proxy média {host} → HTTP {}", status.as_u16());
	}
	let mut builder = Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap());

	// Certains CDN renvoient `application/octet-stream` : la WebView Android
	// refuse alors de lire la video. On retombe sur le type deduit de l'URL.
	let upstream_type = response
		.headers()
		.get(header::CONTENT_TYPE)
		.and_then(|value| value.to_str().ok())
		.map(str::trim)
		.filter(|value| !value.is_empty() && !value.starts_with("application/octet-stream"));
	let content_type = upstream_type
		.map(str::to_string)
		.or_else(|| stream_mime(&src).map(str::to_string));
	if let Some(value) = content_type {
		builder = builder.header(header::CONTENT_TYPE, value);
	}
	for name in ["content-range", "accept-ranges", "content-length"] {
		if let Some(value) = response.headers().get(name) {
			builder = builder.header(name, value);
		}
	}
	builder = builder
		.header(header::CACHE_CONTROL, "no-store")
		.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");

	let body = axum::body::Body::from_stream(response.bytes_stream());
	builder
		.body(body)
		.map_err(|error| ApiError::new(500, format!("Proxy impossible : {error}")))
}
