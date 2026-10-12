//! Routes des téléchargements (`/api/downloads`).

use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::error::{ApiError, Result};
use crate::services::downloads::{
	file_path_of, is_playable_file, manager, mime_for_file, public_entry,
};

pub fn router() -> Router {
	Router::new()
		.route("/", get(list).post(create))
		.route(
			"/{id}",
			get(get_one).delete(remove),
		)
		.route("/{id}/retry", post(retry))
		.route("/{id}/cancel", post(cancel))
		.route("/{id}/stream", get(stream))
		.route("/{id}/file", get(file))
}

async fn list() -> Json<Value> {
	Json(json!({
		"items": manager().list(),
		"summary": manager().summary(),
	}))
}

async fn create(Json(body): Json<Value>) -> std::result::Result<(StatusCode, Json<Value>), ApiError> {
	let entry = manager().create(&body).await?;
	Ok((StatusCode::CREATED, Json(entry)))
}

async fn get_one(Path(id): Path<String>) -> Result<Json<Value>> {
	Ok(Json(public_entry(&manager().entry(&id)?)))
}

async fn retry(Path(id): Path<String>) -> Result<Json<Value>> {
	Ok(Json(manager().retry(&id)?))
}

async fn cancel(Path(id): Path<String>) -> Result<Json<Value>> {
	Ok(Json(manager().cancel(&id)?))
}

async fn remove(Path(id): Path<String>) -> Result<Json<Value>> {
	Ok(Json(manager().remove(&id)?))
}

async fn stream(
	Path(id): Path<String>,
	request: axum::extract::Request,
) -> Result<Response> {
	let range = range_header(&request);
	serve(&id, range, false).await
}

async fn file(
	Path(id): Path<String>,
	request: axum::extract::Request,
) -> Result<Response> {
	let range = range_header(&request);
	serve(&id, range, true).await
}

fn range_header(request: &axum::extract::Request) -> Option<String> {
	request
		.headers()
		.get(header::RANGE)
		.and_then(|value| value.to_str().ok())
		.map(|value| value.trim().to_string())
}

/// Sert un fichier téléchargé (lecture inline ou enregistrement), avec support de `Range`.
async fn serve(id: &str, range: Option<String>, download: bool) -> Result<Response> {
	let entry = manager().entry(id)?;
	let path = file_path_of(&entry);

	if entry.status != "completed" || !path.exists() {
		return Err(ApiError::new(
			409,
			"Ce téléchargement n'est pas encore terminé",
		));
	}

	let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
	let mut headers = vec![
		(
			header::CONTENT_TYPE,
			header::HeaderValue::from_static(mime_for_file(&entry.file_name)),
		),
		(header::ACCEPT_RANGES, header::HeaderValue::from_static("bytes")),
		(
			header::CACHE_CONTROL,
			header::HeaderValue::from_static("private, max-age=0"),
		),
		(
			header::CONTENT_DISPOSITION,
			header::HeaderValue::from_str(&format!(
				"{}; filename*=UTF-8''{}",
				if download { "attachment" } else { "inline" },
				crate::services::providers::urlencode(&entry.file_name)
			))
			.unwrap_or_else(|_| header::HeaderValue::from_static("inline")),
		),
	];

	if let Some(range) = range {
		if let Some((start, end)) = parse_range(&range, size) {
			if start >= size || start > end {
				let mut response = Response::new(axum::body::Body::empty());
				*response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
				response.headers_mut().insert(
					header::CONTENT_RANGE,
					header::HeaderValue::from_str(&format!("bytes */{size}")).unwrap(),
				);
				return Ok(response);
			}

			let length = end - start + 1;
			headers.push((
				header::CONTENT_RANGE,
				header::HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")).unwrap(),
			));
			headers.push((
				header::CONTENT_LENGTH,
				header::HeaderValue::from_str(&length.to_string()).unwrap(),
			));

			let body = read_body(path, start, length).await?;
			let mut response = Response::new(body);
			*response.status_mut() = StatusCode::PARTIAL_CONTENT;
			for (name, value) in headers {
				response.headers_mut().insert(name, value);
			}
			return Ok(response);
		}
	}

	headers.push((
		header::CONTENT_LENGTH,
		header::HeaderValue::from_str(&size.to_string()).unwrap(),
	));
	let body = read_body(path, 0, size).await?;
	let mut response = Response::new(body);
	for (name, value) in headers {
		response.headers_mut().insert(name, value);
	}
	Ok(response)
}

/// Analyse un en-tête `Range: bytes=a-b` (bornes absentes possibles).
fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
	let rest = value.strip_prefix("bytes=")?;
	let (raw_start, raw_end) = rest.split_once('-')?;
	if raw_start.is_empty() && raw_end.is_empty() {
		return None;
	}

	if raw_start.is_empty() {
		// `bytes=-N` : les N derniers octets.
		let suffix: u64 = raw_end.parse().ok()?;
		let start = size.saturating_sub(suffix);
		return Some((start, size.saturating_sub(1)));
	}

	let start: u64 = raw_start.parse().ok()?;
	let end = if raw_end.is_empty() {
		size.saturating_sub(1)
	} else {
		raw_end.parse::<u64>().ok()?.min(size.saturating_sub(1))
	};
	Some((start, end))
}

async fn read_body(path: std::path::PathBuf, start: u64, length: u64) -> Result<axum::body::Body> {
	use futures::TryStreamExt;
	use tokio::io::{AsyncReadExt, AsyncSeekExt};

	let file = std::fs::File::open(&path)
		.map_err(|error| ApiError::new(500, format!("Lecture impossible : {error}")))?;
	let mut file = tokio::fs::File::from_std(file);
	if start > 0 {
		if let Err(error) = file.seek(std::io::SeekFrom::Start(start)).await {
			return Err(ApiError::new(500, format!("Lecture impossible : {error}")));
		}
	}

	let limited = file.take(length);
	let stream = tokio_util::io::ReaderStream::new(limited);
	Ok(axum::body::Body::from_stream(stream.map_err(|error| {
		std::io::Error::other(error)
	})))
}
