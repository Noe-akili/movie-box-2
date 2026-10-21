//! Routes du partage Wi‑Fi local (`/api/transfer`).

use axum::extract::Path;
use axum::http::{header, HeaderMap};
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde_json::Value;

use crate::error::Result;
use crate::services::transfer;

pub fn router() -> Router {
	Router::new()
		.route("/", get(status))
		.route("/start", post(start))
		.route("/stop", post(stop))
		.route("/files", put(set_files))
		.route("/received", delete(clear))
		.route("/received/{id}", delete(remove))
		// Rejoindre une salle existante depuis l'app (QR code ou adresse).
		.route("/join", post(join))
		.route("/leave", post(leave))
		.route("/remote", get(remote_files))
		.route("/remote/file/{id}", get(remote_download))
		.route("/remote/share", post(remote_share))
}

async fn status() -> Json<Value> {
	Json(transfer::status())
}

async fn start(Json(body): Json<Value>) -> Result<Json<Value>> {
	Ok(Json(transfer::start(&body).await?))
}

async fn stop() -> Json<Value> {
	Json(transfer::stop())
}

async fn set_files(Json(body): Json<Value>) -> Result<Json<Value>> {
	Ok(Json(transfer::set_shared(&body)?))
}

async fn clear() -> Json<Value> {
	Json(transfer::clear_received())
}

async fn remove(Path(id): Path<String>) -> Result<Json<Value>> {
	Ok(Json(transfer::remove_received(&id)?))
}

async fn join(Json(body): Json<Value>) -> Result<Json<Value>> {
	Ok(Json(transfer::join(&body).await?))
}

async fn leave() -> Json<Value> {
	Json(transfer::leave())
}

async fn remote_files() -> Result<Json<Value>> {
	Ok(Json(transfer::remote_files().await?))
}

async fn remote_download(Path(id): Path<String>, headers: HeaderMap) -> Result<Response> {
	let range = headers
		.get(header::RANGE)
		.and_then(|value| value.to_str().ok())
		.map(|value| value.trim().to_string());
	transfer::remote_download(&id, range).await
}

async fn remote_share(Json(body): Json<Value>) -> Result<Json<Value>> {
	Ok(Json(transfer::remote_share(&body).await?))
}
