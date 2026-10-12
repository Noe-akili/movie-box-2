//! Point d'entrée du backend Movie Box en Rust.
//!
//! Reproduit `server/src/index.js` : API sous `/api` (rate limit + cache navigateur),
//! frontend compilé (dossier `dist/`) servi par le même serveur, fallback SPA pour
//! Vue Router en mode history, puis gestion centralisée des erreurs.

mod cache;
mod config;
mod error;
mod http;
mod middleware;
mod routes;
mod services;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use axum::body::Body;
use axum::http::header;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde_json::json;
use tower_http::services::ServeDir;

use crate::config::config;

#[tokio::main]
async fn main() {
	if config().tmdb_api_key.is_empty() {
		eprintln!("[movie-box] ⚠️  TMDB_API_KEY absent : les rails TMDB seront indisponibles.");
	}

	let port = config().port;
	let dist_dir = config().dist_dir.clone();
	let has_build = dist_dir.join("index.html").exists();

	// Prépare le dossier de stockage et relance les téléchargements interrompus.
	crate::services::downloads::manager().init();

	let mut app = Router::new()
		.nest("/api", crate::routes::api::router())
		.fallback_service(app_router(dist_dir.clone()))
		.layer(
			tower_http::cors::CorsLayer::new()
				.allow_origin(tower_http::cors::AllowOrigin::any())
				.allow_methods(tower_http::cors::AllowMethods::any())
				.allow_headers(tower_http::cors::AllowHeaders::any())
				.allow_credentials(false),
		)
		.layer(axum::middleware::from_fn(crate::middleware::security_headers))
		.layer(axum::middleware::from_fn(crate::middleware::rate_limit))
		.layer(axum::middleware::from_fn(crate::middleware::api_cache_control));

	// Sans build frontend, `/` décrit l'API ; sinon `dist/index.html` est servi par le fallback.
	if !has_build {
		app = app.route("/", get(root));
	}

	let addr = SocketAddr::from(([0, 0, 0, 0], port));
	println!("[movie-box] écoute sur http://{addr}");
	let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
	axum::serve(listener, app.into_make_service()).await.expect("serveur");
}

/// Page d'accueil : liste les endpoints quand le frontend n'est pas compilé.
async fn root() -> Response {
	let body = json!({
		"name": "Movie Box API",
		"runtime": "rust",
		"status": "ok",
		"endpoints": [
			"GET /api/health",
			"GET /api/home",
			"GET /api/genres?type=movie|tv",
			"GET /api/search?q=",
			"GET /api/discover/movie|tv",
			"GET /api/free?collection=&q=&page=",
			"GET /api/title/:mediaType/:id",
			"GET /api/title/tv/:id/season/:n",
			"GET /api/downloads",
		],
	});
	(Response::builder()
		.status(200)
		.header(header::CONTENT_TYPE, "application/json")
		.body(Body::from(serde_json::to_string(&body).unwrap())))
		.unwrap()
}

/// Frontend compilé (dossier `dist/`) + fallback SPA (Vue Router en mode history).
/// `ServeDir` sert les assets existants, le fallback renvoie `index.html` pour le
/// routage côté client lorsqu'un build est présent, sinon 404 JSON.
fn app_router(dist_dir: PathBuf) -> Router {
	let index = dist_dir.join("index.html");
	if index.exists() {
		// Les assets existants sont servis tels quels ; toute route inconnue renvoie
		// `index.html` (statut 200) pour laisser Vue Router (mode history) prendre le relais.
		let fallback_index = index.clone();
		let serve = ServeDir::new(&dist_dir)
			.precompressed_gzip()
			.precompressed_br()
			.fallback(get(move || {
				let index = fallback_index.clone();
				async move {
					match tokio::fs::read(&index).await {
						Ok(bytes) => Response::builder()
							.status(200)
							.header(header::CONTENT_TYPE, "text/html; charset=utf-8")
							.body(Body::from(bytes))
							.unwrap(),
						Err(error) => {
							let body = json!({ "error": { "status": 500, "message": format!("Lecture de index.html impossible : {error}") } });
							Response::builder()
								.status(500)
								.header(header::CONTENT_TYPE, "application/json")
								.body(Body::from(serde_json::to_string(&body).unwrap()))
								.unwrap()
						}
					}
				}
			}));
		Router::new().fallback_service(serve)
	} else {
		Router::new().route(
			"/*_",
			get(|| async move {
				let body = json!({ "error": { "status": 404, "message": "Frontend absent : lance « npm run dev » (frontend Vite) ou « npm run build »." } });
				(Response::builder()
					.status(404)
					.header(header::CONTENT_TYPE, "application/json")
					.body(Body::from(serde_json::to_string(&body).unwrap())))
					.unwrap()
			}),
		)
	}
}
