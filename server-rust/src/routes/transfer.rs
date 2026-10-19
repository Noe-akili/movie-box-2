//! Routes du partage Wi‑Fi local (`/api/transfer`).

use axum::extract::Path;
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
