//! Routes des téléchargements (`/api/downloads`).

use axum::extract::Path;
use axum::http::header;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::error::{ApiError, Result};
use crate::services::downloads::{file_path_of, manager, public_entry, serve_path};

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

async fn create(Json(body): Json<Value>) -> std::result::Result<(axum::http::StatusCode, Json<Value>), ApiError> {
	let entry = manager().create(&body).await?;
	Ok((axum::http::StatusCode::CREATED, Json(entry)))
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

	if entry.status != "completed" {
		return Err(ApiError::new(
			409,
			"Ce téléchargement n'est pas encore terminé",
		));
	}

	serve_path(file_path_of(&entry), &entry.file_name, range, download).await
}
