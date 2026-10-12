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

	let mut upstream = crate::http::client()
		.get(url)
		.header("user-agent", "ExoPlayerLib/2.18.7")
		.header("accept", "*/*")
		// Le CDN video (bcdnxw.hakunaymatata.com) applique une protection
		// anti-hotlink qui repond 429 sauf pour certains referers autorises.
		// `movieboxapp.in` est l'un d'eux : sans cet en-tete tous les flux
		// videos sont refuses.
		.header("referer", "https://movieboxapp.in/");
	if !cookie.is_empty() {
		upstream = upstream.header("cookie", cookie);
	}
	if let Some(range) = request
		.headers()
		.get(header::RANGE)
		.and_then(|value| value.to_str().ok())
	{
		upstream = upstream.header("range", range);
	}

	let response = upstream
		.send()
		.await
		.map_err(|_| ApiError::upstream(504, "Flux distant injoignable"))?;

	let status = response.status();
	let mut builder = Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap());

	if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
		builder = builder.header(header::CONTENT_TYPE, content_type);
	}
	for name in ["content-range", "accept-ranges", "content-length"] {
		if let Some(value) = response.headers().get(name) {
			builder = builder.header(name, value);
		}
	}
	builder = builder.header(header::CACHE_CONTROL, "no-store");

	let body = axum::body::Body::from_stream(response.bytes_stream());
	builder
		.body(body)
		.map_err(|error| ApiError::new(500, format!("Proxy impossible : {error}")))
}
