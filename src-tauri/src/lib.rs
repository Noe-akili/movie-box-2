//! Application Tauri (Android / desktop) pour Movie Box.
//!
//! Le backend Rust (`movie-box-server`) est appelé principalement via deux
//! canaux internes à Tauri —
//! 1. l'IPC (`invoke("api_request")`) pour toutes les requêtes JSON ;
//! 2. un protocole d'URL personnalisé (`moviebox://`) pour les flux binaires
//!    (lecture vidéo, fichiers téléchargés) qui ont besoin d'une vraie URL.
//!
//! Sur Android, le protocole personnalisé ne sait pas honorer les requêtes
//! `Range` : la WebView ré-applique la plage demandée au corps 206 déjà découpé
//! (bug wry #1864), si bien que la lecture redémarre à chaque avance. On expose
//! donc en plus un petit serveur HTTP sur `127.0.0.1` (port éphémère) qui, lui,
//! gère correctement les plages et permet le seek. Le reste des plateformes
//! continue d'utiliser `moviebox://`.

mod diag;

use axum::body::Body;
use std::sync::OnceLock;
use tauri::http::{Request as HttpRequest, Response as HttpResponse};
use tauri::Manager;
use tower::ServiceExt;

/// Origine virtuelle du backend, exposée au frontend (`invoke("server_origin")`).
///
/// Sur Android et Windows, un protocole personnalisé est servi sous la forme
/// `http://<scheme>.localhost` ; sur macOS/Linux sous la forme `<scheme>://localhost`.
#[cfg(any(target_os = "android", target_os = "windows"))]
const PROTOCOL_ORIGIN: &str = "http://moviebox.localhost";
#[cfg(not(any(target_os = "android", target_os = "windows")))]
const PROTOCOL_ORIGIN: &str = "moviebox://localhost";

/// Origine du serveur média HTTP loopback (`http://127.0.0.1:<port>`), publiée
/// par `setup()` sur Android. Tant qu'elle est vide, on retombe sur le protocole
/// personnalisé `PROTOCOL_ORIGIN`.
static MEDIA_ORIGIN: OnceLock<String> = OnceLock::new();

/// Origine média effective : loopback HTTP si disponible, sinon `moviebox://`.
fn effective_origin() -> String {
	MEDIA_ORIGIN
		.get()
		.cloned()
		.unwrap_or_else(|| PROTOCOL_ORIGIN.to_string())
}

/// En-têtes relayés depuis la WebView vers le routeur (le reste est ignoré :
/// `host`, `content-length`, etc. n'ont pas de sens pour un routeur interne).
const FORWARDED_HEADERS: [&str; 6] = [
	"range",
	"if-range",
	"if-none-match",
	"if-modified-since",
	"accept",
	"accept-language",
];

/// Backend partagé : le routeur Axum de l'API (catalogue, source interne, proxy).
///
/// L'état est déclaré dès le `Builder` (`Builder::manage`), donc **avant** la
/// création de la fenêtre/WebView : Tauri crée les fenêtres avant le hook
/// `setup()`. Une requête `moviebox://` arrivant pendant la initialisation trouve
/// ici un `OnceLock` vide et reçoit une 503 — au lieu de `state::<Backend>()`
/// qui, si l'état n'était pas géré, ferait paniquer le thread JNI et **fermerait
/// l'application Android** (panic = abort via `stop_unwind`).
struct Backend {
	router: OnceLock<axum::Router>,
}

/// Réponse JSON de l'IPC.
#[derive(serde::Serialize)]
struct ApiResponse {
	status: u16,
	body: String,
}

/// Origine du backend embarqué (utilisée pour construire les URLs média).
#[tauri::command]
fn server_origin() -> String {
	effective_origin()
}

/// Exécute une requête de l'API contre le routeur Axum, sans passer par le réseau.
#[tauri::command]
async fn api_request(
	backend: tauri::State<'_, Backend>,
	method: String,
	path: String,
	query: Option<String>,
	body: Option<String>,
) -> Result<ApiResponse, String> {
	let label = format!("{method} {path}");

	let router = match backend.router.get().cloned() {
		Some(router) => router,
		None => {
			let message = "backend en cours de démarrage, réessaie dans un instant".to_string();
			diag::log(format!("api_request {label} refusée : routeur absent"));
			return Err(message);
		}
	};

	let uri = match query {
		Some(query) if !query.is_empty() => format!("{path}?{query}"),
		_ => path,
	};

	let method = method
		.parse::<axum::http::Method>()
		.map_err(|error| error.to_string())?;

	let request = axum::http::Request::builder()
		.method(method)
		.uri(uri)
		.header(axum::http::header::CONTENT_TYPE, "application/json")
		.body(Body::from(body.unwrap_or_default()))
		.map_err(|error| error.to_string())?;

	let response = match router.oneshot(request).await {
		Ok(response) => response,
		Err(_) => return Err("routeur API injoignable".to_string()),
	};

	let status = response.status().as_u16();
	let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024 * 1024)
		.await
		.map_err(|error| error.to_string())?;

	if status >= 400 {
		diag::log(format!("api_request {label} -> {status}"));
	}

	Ok(ApiResponse {
		status,
		body: String::from_utf8_lossy(&bytes).into_owned(),
	})
}

/// Rapport de diagnostic exposé au panneau développeur du frontend.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagReport {
	/// Journal mémoire (démarrage, paniques, erreurs IPC).
	log: String,
	/// Origine média virtuelle (`http://moviebox.localhost` sur Android).
	server_origin: String,
	/// Vrai dès que le routeur Axum a été publié par `setup()`.
	router_ready: bool,
	platform: String,
	tauri_version: String,
}

/// Renvoie l'état interne du backend pour le mode développeur de l'interface.
#[tauri::command]
fn backend_diag(backend: tauri::State<'_, Backend>) -> DiagReport {
	DiagReport {
		log: diag::snapshot(),
		server_origin: effective_origin(),
		router_ready: backend.router.get().is_some(),
		platform: std::env::consts::OS.to_string(),
		tauri_version: tauri::VERSION.to_string(),
	}
}

/// Relaie une requête du protocole `moviebox://` vers le routeur Axum.
///
/// Utilisé pour les flux binaires (vidéo, fichiers) : on conserve les en-têtes
/// `Range`/`If-*` et on renvoie le corps complet (borne imposée par la plage).
async fn handle_protocol_request(
	router: axum::Router,
	request: HttpRequest<Vec<u8>>,
) -> HttpResponse<Vec<u8>> {
	let method = request.method().clone();
	let path_and_query = request
		.uri()
		.path_and_query()
		.map(|value| value.as_str().to_string())
		.unwrap_or_else(|| "/".to_string());

	let mut builder = axum::http::Request::builder().method(method).uri(path_and_query);
	if let Some(headers) = builder.headers_mut() {
		for name in FORWARDED_HEADERS {
			if let Some(value) = request.headers().get(name) {
				headers.insert(name, value.clone());
			}
		}
	}

	let axum_request = match builder.body(Body::from(request.into_body())) {
		Ok(request) => request,
		Err(error) => {
			return HttpResponse::builder()
				.status(400)
				.body(format!("requête média invalide : {error}").into_bytes())
				.unwrap();
		}
	};

	let response = match router.oneshot(axum_request).await {
		Ok(response) => response,
		Err(_) => {
			return HttpResponse::builder()
				.status(500)
				.body("routeur média injoignable".as_bytes().to_vec())
				.unwrap();
		}
	};

	let (parts, body) = response.into_parts();
	let bytes = match axum::body::to_bytes(body, usize::MAX).await {
		Ok(bytes) => bytes,
		Err(error) => {
			return HttpResponse::builder()
				.status(502)
				.body(format!("flux média interrompu : {error}").into_bytes())
				.unwrap();
		}
	};

	let mut out = HttpResponse::builder().status(parts.status);
	if let Some(headers) = out.headers_mut() {
		for (name, value) in &parts.headers {
			headers.insert(name.clone(), value.clone());
		}
		// Les flux par plages ne doivent jamais être servis depuis un cache :
		// une plage mise en cache pour une autre plage donnerait des octets faux.
		headers.insert(
			axum::http::header::CACHE_CONTROL,
			axum::http::HeaderValue::from_static("no-store"),
		);
	}

	out.body(bytes.to_vec()).unwrap()
}

/// Démarre le serveur média HTTP loopback (cf. commentaire de module).
///
/// Compilé sur toutes les plateformes pour rester vérifiable localement, mais
/// appelé uniquement sur Android : ailleurs le protocole `moviebox://` suffit.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn spawn_media_server<R: tauri::Runtime>(app: &tauri::App<R>) {
	let router = match app.state::<Backend>().router.get().cloned() {
		Some(router) => router,
		None => {
			diag::log("serveur média loopback : routeur absent, démarrage ignoré");
			return;
		}
	};

	let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
		Ok(listener) => listener,
		Err(error) => {
			diag::log(format!("serveur média loopback : bind impossible — {error}"));
			return;
		}
	};

	let addr = match listener.local_addr() {
		Ok(addr) => addr,
		Err(error) => {
			diag::log(format!("serveur média loopback : adresse inconnue — {error}"));
			return;
		}
	};

	let origin = format!("http://127.0.0.1:{}", addr.port());
	let _ = MEDIA_ORIGIN.set(origin.clone());
	diag::log(format!("setup : serveur média loopback = {origin}"));

	let _ = listener.set_nonblocking(true);
	tauri::async_runtime::spawn(async move {
		match tokio::net::TcpListener::from_std(listener) {
			Ok(listener) => {
				if let Err(error) = axum::serve(listener, router.into_make_service()).await {
					diag::log(format!("serveur média loopback arrêté : {error}"));
				}
			}
			Err(error) => diag::log(format!("serveur média loopback (tokio) : {error}")),
		}
	});
}

/// Garde le processus éveillé quand l'application passe en arrière-plan
/// (service de premier plan Android) : les téléchargements continuent côté
/// backend. Ce service ne fait aucun travail lui-même, il maintient juste la
/// durée de vie du processus jusqu'à l'arrêt demandé par l'interface.
struct DownloadsKeepAlive;

#[async_trait::async_trait]
impl<R: tauri::Runtime> tauri_plugin_background_service::BackgroundService<R>
	for DownloadsKeepAlive
{
	async fn init(
		&mut self,
		_ctx: &tauri_plugin_background_service::ServiceContext<R>,
	) -> std::result::Result<(), tauri_plugin_background_service::ServiceError> {
		Ok(())
	}

	async fn run(
		&mut self,
		ctx: &tauri_plugin_background_service::ServiceContext<R>,
	) -> std::result::Result<(), tauri_plugin_background_service::ServiceError> {
		ctx.shutdown.cancelled().await;
		Ok(())
	}
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	// Le crochet est posé avant tout : une panique (même rattrapée plus bas)
	// laisse une trace consultable dans le panneau développeur.
	diag::install_panic_hook();
	diag::log(format!(
		"démarrage movie-box (tauri {}, os {})",
		tauri::VERSION,
		std::env::consts::OS
	));

	tauri::Builder::default()
		// Notifications (utilisées par le service de premier plan) puis service
		// d'arrière-plan : les téléchargements survivent à la mise en veille de
		// la WebView sur Android.
		.plugin(tauri_plugin_notification::init())
		.plugin(tauri_plugin_background_service::init_with_service(|| {
			DownloadsKeepAlive
		}))
		// État déclaré AVANT la création de la fenêtre : le gestionnaire du
		// protocole ne peut jamais paniquer sur un état absent (cf. `Backend`).
		.manage(Backend {
			router: OnceLock::new(),
		})
		.register_asynchronous_uri_scheme_protocol("moviebox", |context, request, responder| {
			let router = context.app_handle().state::<Backend>().router.get().cloned();
			match router {
				Some(router) => {
					tauri::async_runtime::spawn(async move {
						let response = handle_protocol_request(router, request).await;
						responder.respond(response);
					});
				}
				// Routeur pas encore publié : on répond au lieu de planter, la
				// WebView retentera naturellement (images, plages vidéo).
				None => {
					let response = HttpResponse::builder()
						.status(503)
						.header(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
						.body(b"backend en cours de d\xE9marrage".to_vec());
					match response {
						Ok(response) => responder.respond(response),
						Err(_) => responder.respond(HttpResponse::new(Vec::new())),
					}
				}
			}
		})
		.setup(|app| {
			// GARANTIE : une panique pendant l'initialisation ne doit JAMAIS
			// fermer l'application. `tauri::mobile_entry_point` enveloppe tout
			// `run()` dans `stop_unwind`, qui transforme la moindre panique du
			// thread principal en `std::process::abort()` — soit une fermeture
			// immédiate de l'APK au démarrage (WebView Android fragile au boot).
			// On rattrape donc l'init ici, comme le projet frère `game-lounge`.
			let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				diag::log("setup : configuration de l'environnement embarqué");
				// Sur Android, aucun `.env` n'existe : toute la configuration du serveur
				// est injectée ici, AVANT le premier appel à `config()`.
				let data_dir = app
					.path()
					.app_data_dir()
					.unwrap_or_else(|_| std::env::temp_dir().join("movie-box"));
				let _ = std::fs::create_dir_all(&data_dir);

				diag::log(format!("setup : dossier de données = {}", data_dir.display()));
				std::env::set_var("DOWNLOADS_DIR", data_dir.join("downloads"));
				std::env::set_var("INTERNAL_API_ENABLED", "true");

				if let Some(key) = option_env!("MOVIEBOX_TMDB_API_KEY") {
					if !key.is_empty() {
						std::env::set_var("TMDB_API_KEY", key);
					}
				}

				// Le routeur est construit une fois puis partagé par l'IPC et le
				// protocole média. L'état `Backend` est déjà géré par le Builder
				// (donc toujours présent) ; on ne fait que le publier ici (la
				// fenêtre/WebView étant créée avant ce hook, une requête trop
				// précoce reçoit une 503 au lieu de planter).
				let dist_dir = data_dir.join("web");
				diag::log(format!(
					"setup : construction du routeur (dist={})",
					dist_dir.display()
				));
				let router = movie_box_server::app(dist_dir);
				let _ = app.state::<Backend>().router.set(router);
				diag::log("setup : routeur publié");

				// Serveur média HTTP loopback (Android) : le protocole
				// personnalisé ne gère pas les requêtes `Range` (seek), un vrai
				// socket local si. Le port est éphémère, l'écoute reste locale.
				#[cfg(target_os = "android")]
				spawn_media_server(app);
			}));

			match result {
				Ok(()) => diag::log("setup : terminé"),
				Err(payload) => diag::log(format!(
					"setup : panique rattrapée — {}",
					diag::payload_text(payload.as_ref())
				)),
			}

			// Reprise des téléchargements (tâches tokio) dans le runtime de Tauri.
			// Isolée dans sa propre tâche et protégée : une panique ici ne doit
			// pas remonter au thread principal (=> abort).
			tauri::async_runtime::spawn(async move {
				diag::log("reprise des téléchargements…");
				match std::panic::catch_unwind(std::panic::AssertUnwindSafe(
					movie_box_server::init_state,
				)) {
					Ok(()) => diag::log("téléchargements repris"),
					Err(payload) => diag::log(format!(
						"reprise des téléchargements : panique — {}",
						diag::payload_text(payload.as_ref())
					)),
				}
			});

			Ok(())
		})
		.invoke_handler(tauri::generate_handler![server_origin, api_request, backend_diag])
		.run(tauri::generate_context!())
		.unwrap_or_else(|error| {
			// stdout/stderr de l'app sont redirigés vers logcat (tag
			// `RustStdoutStderr`) : le message reste visible avec `adb logcat`.
			diag::log(format!("erreur au lancement : {error}"));
			std::process::abort()
		});
}
