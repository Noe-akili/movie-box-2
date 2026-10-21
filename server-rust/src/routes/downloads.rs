//! Routes des téléchargements (`/api/downloads`).

use axum::extract::Path;
use axum::http::header;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::error::{ApiError, Result};
use crate::services::downloads::{manager, playback_path_of, public_entry, serve_path};

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
	serve(&id, range, false, true).await
}

async fn file(
	Path(id): Path<String>,
	request: axum::extract::Request,
) -> Result<Response> {
	let range = range_header(&request);
	serve(&id, range, true, false).await
}

fn range_header(request: &axum::extract::Request) -> Option<String> {
	request
		.headers()
		.get(header::RANGE)
		.and_then(|value| value.to_str().ok())
		.map(|value| value.trim().to_string())
}

/// Sert un fichier téléchargé (lecture inline ou enregistrement), avec support de `Range`.
///
/// La lecture accepte un transfert inachevé en servant la partie déjà reçue ;
/// l'enregistrement sur l'appareil attend, lui, la fin du téléchargement.
async fn serve(
	id: &str,
	range: Option<String>,
	download: bool,
	allow_partial: bool,
) -> Result<Response> {
	let entry = manager().entry(id)?;

	if !allow_partial && entry.status != "completed" {
		return Err(ApiError::new(
			409,
			"Ce téléchargement n'est pas encore terminé",
		));
	}

	let path = playback_path_of(&entry).ok_or_else(|| {
		ApiError::new(409, "Ce téléchargement n'a pas encore de données lisibles")
	})?;
	serve_path(path, &entry.file_name, range, download).await
}
