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

/// Politique de cache : courte sur /api, immuable sur les assets hachés, revalidation sur le HTML.
pub async fn api_cache_control(req: Request, next: Next) -> Response {
	let path = req.uri().path();
	// Les flux (téléchargements et proxy média par plages d'octets) ne doivent
	// jamais être mis en cache.
	let cache_control = if path.starts_with("/api/downloads")
		|| path.starts_with("/api/internal/proxy")
		|| path.starts_with("/api/transfer")
	{
		"no-store"
	} else if path.starts_with("/api/free") {
		// Pages de catalogue libre : contenu peu changeant, cache plus long.
		"public, max-age=300"
	} else if path.starts_with("/api/home") {
		"public, max-age=120"
	} else if path.starts_with("/api/") {
		// Catalogue : cache court, suffisant pour absorber un rafraîchissement.
		"public, max-age=60"
	} else if path.starts_with("/assets/") {
		// Assets compilés par Vite : nom hashé → contenu immuable, cache long.
		"public, max-age=31536000, immutable"
	} else {
		// index.html et routes SPA : toujours revalider pour ne pas servir un vieux build.
		"no-cache"
	};

	let mut response = next.run(req).await;
	response.headers_mut().insert(
		HeaderName::from_static("cache-control"),
		HeaderValue::from_static(cache_control),
	);

	response
}
