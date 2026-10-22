//! Backend Movie Box en Rust, exposé comme bibliothèque.
//!
//! Reproduit `server/src/index.js` : API sous `/api` (rate limit + cache navigateur),
//! frontend compilé (dossier `dist/`) servi par le même serveur, fallback SPA pour
//! Vue Router en mode history, puis gestion centralisée des erreurs.
//!
//! La bibliothèque est partagée entre deux consommateurs :
//! - le binaire `movie-box-server` (serveur autonome, `main.rs`) ;
//! - l'application Tauri Android, qui embarque le serveur dans le processus de
//!   l'app et écoute sur `127.0.0.1`.

pub mod cache;
pub mod config;
pub mod error;
pub mod http;
pub mod middleware;
pub mod routes;
pub mod services;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::body::Body;
use axum::http::header;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use serde_json::json;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

use crate::config::config;

/// Initialise l'état partagé : dossier de stockage + reprise des téléchargements.
pub fn init_state() {
	crate::services::downloads::manager().init();
}

/// Application complète (API + fichiers statiques) prête à être servie.
pub fn app(dist_dir: PathBuf) -> Router {
	if config().tmdb_api_key.is_empty() {
		eprintln!("[movie-box] ⚠️  TMDB_API_KEY absent : les rails TMDB seront indisponibles.");
	}

	let has_build = dist_dir.join("index.html").exists();

	let mut app = Router::new()
		.nest("/api", crate::routes::api::router())
		.fallback_service(app_router(dist_dir))
		.layer(
			tower_http::cors::CorsLayer::new()
				.allow_origin(tower_http::cors::AllowOrigin::any())
				.allow_methods(tower_http::cors::AllowMethods::any())
				.allow_headers(tower_http::cors::AllowHeaders::any())
				.allow_credentials(false),
		)
		.layer(axum::middleware::from_fn(crate::middleware::security_headers))
		.layer(axum::middleware::from_fn(crate::middleware::rate_limit))
		.layer(axum::middleware::from_fn(
			crate::middleware::api_cache_control,
		))
		// Les réponses JSON (catalogues, accueil, recherche) sont volumineuses :
		// sans compression, un téléphone en 3G télécharge plusieurs fois plus
		// d'octets que nécessaire.
		.layer(CompressionLayer::new());

	// Sans build frontend, `/` décrit l'API ; sinon `dist/index.html` est servi par le fallback.
	if !has_build {
		app = app.route("/", get(root));
	}

	app
}

/// Sert l'application sur un listener déjà lié (utilisé par Tauri).
pub async fn serve(
	listener: tokio::net::TcpListener,
	dist_dir: PathBuf,
) -> std::io::Result<()> {
	axum::serve(listener, app(dist_dir).into_make_service()).await
}

/// Lance le serveur autonome sur `0.0.0.0:PORT` (binaire `movie-box-server`).
pub async fn run_standalone() {
	init_state();

	let port = config().port;
	let dist_dir = config().dist_dir.clone();
	let addr = SocketAddr::from(([0, 0, 0, 0], port));

	println!("[movie-box] écoute sur http://{addr}");
	let listener = tokio::net::TcpListener::bind(addr)
		.await
		.expect("bind");
	serve(listener, dist_dir).await.expect("serveur");
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
			"/{*_}",
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
