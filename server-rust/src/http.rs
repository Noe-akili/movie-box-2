//! Client HTTP amont : fetch JSON + timeout + retry, avec erreurs API propres.

use std::time::Duration;

use serde_json::Value;

use crate::error::{ApiError, Result};

const DEFAULT_TIMEOUT_MS: u64 = 12_000;

static CLIENT: std::sync::LazyLock<reqwest::Client> = std::sync::LazyLock::new(|| {
	reqwest::Client::builder()
		.user_agent("movie-box/1.0")
		.connect_timeout(Duration::from_secs(8))
		.build()
		.expect("client HTTP")
});

pub fn client() -> &'static reqwest::Client {
	&CLIENT
}

/// Construit une URL avec des paramètres (les valeurs vides sont ignorées).
pub fn build_url(base: &str, params: &[(&str, String)]) -> String {
	let mut url = match reqwest::Url::parse(base) {
		Ok(url) => url,
		Err(_) => return base.to_string(),
	};

	{
		let mut query = url.query_pairs_mut();
		for (key, value) in params {
			if value.is_empty() {
				continue;
			}
			query.append_pair(key, value);
		}
	}

	url.to_string()
}

fn sleep(ms: u64) -> impl std::future::Future<Output = ()> {
	async move {
		tokio::time::sleep(Duration::from_millis(ms)).await;
	}
}

/// fetch + JSON + timeout + retry (2 tentatives par défaut), comme `lib/http.js`.
pub async fn fetch_json(
	url: &str,
	timeout_ms: u64,
	retries: u32,
	headers: &[(&str, &str)],
) -> Result<Value> {
	let mut last_error: Option<ApiError> = None;
	let mut attempt = 0u32;

	while attempt <= retries {
		let mut request = client()
			.get(url)
			.timeout(Duration::from_millis(timeout_ms))
			.header("accept", "application/json");
		for (key, value) in headers {
			request = request.header(*key, *value);
		}

		match request.send().await {
			Ok(response) => {
				let status = response.status();
				let retry_after: u64 = response
					.headers()
					.get("retry-after")
					.and_then(|value| value.to_str().ok())
					.and_then(|value| value.parse().ok())
					.unwrap_or(0);

				let body = response.text().await.unwrap_or_default();

				if !status.is_success() {
					// Wikimedia et consorts limitent le débit : on respecte Retry-After.
					if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < retries {
						sleep(std::cmp::min(5_000, std::cmp::max(700, retry_after * 1_000))).await;
						attempt += 1;
						continue;
					}

					let mapped = if status == reqwest::StatusCode::NOT_FOUND {
						404
					} else {
						502
					};
					return Err(ApiError::new(
						mapped,
						format!("Service amont indisponible ({})", status.as_u16()),
					));
				}

				if body.is_empty() {
					return Ok(Value::Null);
				}

				match serde_json::from_str::<Value>(&body) {
					Ok(value) => return Ok(value),
					Err(_) => {
						last_error = Some(ApiError::new(
							502,
							"Réponse amont illisible (JSON invalide)",
						));
						if attempt < retries {
							sleep(200 * u64::from(attempt + 1)).await;
							attempt += 1;
							continue;
						}
						return Err(last_error.take().unwrap());
					}
				}
			}
			Err(error) => {
				if error.is_timeout() || error.is_connect() || error.is_request() {
					last_error = Some(ApiError::new(
						504,
						"Délai dépassé lors de l'appel au service amont",
					));
				} else if let Some(status) = error.status() {
					last_error = Some(ApiError::upstream(
						if status == reqwest::StatusCode::NOT_FOUND {
							404
						} else {
							502
						},
						format!("Service amont indisponible ({})", status.as_u16()),
					));
				} else {
					last_error = Some(ApiError::new(
						504,
						"Délai dépassé lors de l'appel au service amont",
					));
				}

				if last_error.as_ref().is_some_and(|error| error.status == 404) {
					return Err(last_error.take().unwrap());
				}
				if attempt < retries {
					sleep(200 * u64::from(attempt + 1)).await;
					attempt += 1;
					continue;
				}
				return Err(last_error.take().unwrap());
			}
		}
	}

	Err(ApiError::new(
		504,
		"Délai dépassé lors de l'appel au service amont",
	))
}

/// Taille réelle d'un fichier distant (requête `Range: bytes=0-0`), pour la progression.
pub async fn probe_remote_size(url: &str, timeout_ms: u64) -> u64 {
	let response = client()
		.get(url)
		.timeout(Duration::from_millis(timeout_ms))
		.header("range", "bytes=0-0")
		.send()
		.await;

	let Ok(response) = response else {
		return 0;
	};
	if !response.status().is_success() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
	{
		return 0;
	}

	let total = response
		.headers()
		.get("content-range")
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.split('/').nth(1))
		.and_then(|value| value.parse::<u64>().ok())
		.unwrap_or(0);

	total
}
