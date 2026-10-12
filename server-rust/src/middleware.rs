//! Middlewares : sécurité, rate limit par IP et cache navigateur court sur /api.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::sync::LazyLock;

use crate::config::config;

/// En-têtes de sécurité posés sur toutes les réponses.
pub async fn security_headers(req: Request, next: Next) -> Response {
	let mut response = next.run(req).await;
	let headers = response.headers_mut();
	headers.insert(
		HeaderName::from_static("x-content-type-options"),
		HeaderValue::from_static("nosniff"),
	);
	headers.insert(
		HeaderName::from_static("referrer-policy"),
		HeaderValue::from_static("no-referrer-when-downgrade"),
	);
	response
}

struct RateEntry {
	count: u32,
	reset_at: Instant,
}

static HITS: LazyLock<Mutex<HashMap<String, RateEntry>>> =
	LazyLock::new(|| Mutex::new(HashMap::new()));

fn client_key(req: &Request) -> String {
	if let Some(forwarded) = req
		.headers()
		.get("x-forwarded-for")
		.and_then(|value| value.to_str().ok())
	{
		if let Some(first) = forwarded.split(',').next() {
			let first = first.trim();
			if !first.is_empty() {
				return first.to_string();
			}
		}
	}

	if let Some(connect_info) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
		return connect_info.0.ip().to_string();
	}

	"unknown".to_string()
}

/// Rate limit mémoire (fenêtre glissante) pour protéger les API amont.
pub async fn rate_limit(req: Request, next: Next) -> Response {
	let config = config();
	let now = Instant::now();
	let key = client_key(&req);

	let (remaining, blocked, retry_after) = {
		let mut hits = HITS.lock().unwrap();
		let window = Duration::from_millis(config.rate_limit.window_ms);

		match hits.get_mut(&key) {
			Some(entry) if entry.reset_at > now => {
				entry.count += 1;
				let remaining = config.rate_limit.max.saturating_sub(entry.count);
				(
					remaining,
					entry.count > config.rate_limit.max,
					entry.reset_at.duration_since(now).as_secs().max(1),
				)
			}
			_ => {
				hits.insert(
					key.clone(),
					RateEntry {
						count: 1,
						reset_at: now + window,
					},
				);
				if hits.len() > 5_000 {
					let expired: Vec<String> = hits
						.iter()
						.filter(|(_, entry)| entry.reset_at <= now)
						.map(|(key, _)| key.clone())
						.collect();
					for key in expired {
						hits.remove(&key);
					}
				}
				(config.rate_limit.max.saturating_sub(1), false, 0)
			}
		}
	};

	if blocked {
		return (
			[("retry-after", retry_after.to_string())],
			Json(json!({ "error": { "status": 429, "message": "Trop de requêtes, merci de patienter." } })),
		)
			.into_response();
	}

	let mut response = next.run(req).await;
	response.headers_mut().insert(
		HeaderName::from_static("x-ratelimit-remaining"),
		HeaderValue::from_str(&remaining.to_string()).unwrap_or(HeaderValue::from_static("0")),
	);
	response
}

/// Cache navigateur court sur /api (les téléchargements ne doivent jamais être mis en cache).
pub async fn api_cache_control(req: Request, next: Next) -> Response {
	let is_downloads = req.uri().path().starts_with("/api/downloads");
	let mut response = next.run(req).await;

	let value = if is_downloads {
		"no-store"
	} else {
		"public, max-age=60"
	};
	response.headers_mut().insert(
		HeaderName::from_static("cache-control"),
		HeaderValue::from_static(if is_downloads { "no-store" } else { "public, max-age=60" }),
	);
	let _ = value;

	response
}
