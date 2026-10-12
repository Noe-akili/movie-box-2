//! Erreurs API : enveloppe JSON homogène `{ error: { status, message } }`.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ApiError {
	pub status: u16,
	pub message: String,
}

pub type Result<T, E = ApiError> = std::result::Result<T, E>;

impl ApiError {
	pub fn new(status: u16, message: impl Into<String>) -> Self {
		Self {
			status,
			message: message.into(),
		}
	}

	pub fn bad_request(message: impl Into<String>) -> Self {
		Self::new(400, message)
	}

	pub fn not_found(message: impl Into<String>) -> Self {
		Self::new(404, message)
	}

	pub fn upstream(status: u16, message: impl Into<String>) -> Self {
		Self::new(status, message)
	}
}

impl std::fmt::Display for ApiError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(formatter, "{}", self.message)
	}
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
	fn into_response(self) -> Response {
		if self.status >= 500 {
			eprintln!("[movie-box] {}", self.message);
		}

		let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
		(
			status,
			Json(json!({ "error": { "status": self.status, "message": self.message } })),
		)
			.into_response()
	}
}
