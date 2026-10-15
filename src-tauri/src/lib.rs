use std::sync::atomic::{AtomicU16, Ordering};
use tauri::Manager;

/// Port réellement occupé par le serveur embarqué (choisi au démarrage).
static SERVER_PORT: AtomicU16 = AtomicU16::new(5174);

/// Origine du backend embarqué, exposée au frontend (`invoke("server_origin")`).
#[tauri::command]
fn server_origin() -> String {
	format!("http://127.0.0.1:{}", SERVER_PORT.load(Ordering::Relaxed))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.setup(|app| {
			// Sur Android, aucun `.env` n'existe : toute la configuration du serveur
			// est injectée ici, AVANT le premier appel à `config()`.
			let data_dir = app
				.path()
				.app_data_dir()
				.unwrap_or_else(|_| std::env::temp_dir().join("movie-box"));
			let _ = std::fs::create_dir_all(&data_dir);

			std::env::set_var("DOWNLOADS_DIR", data_dir.join("downloads"));
			std::env::set_var("INTERNAL_API_ENABLED", "true");

			if let Some(key) = option_env!("MOVIEBOX_TMDB_API_KEY") {
				if !key.is_empty() {
					std::env::set_var("TMDB_API_KEY", key);
				}
			}

			// Liaison SYNCHRONE (avant le chargement de la WebView) sur un port libre :
			// le serveur est joignable dès le premier appel de l'interface, et deux
			// instances de l'app ne se marchent pas dessus.
			let listener = match std::net::TcpListener::bind(("127.0.0.1", 0)) {
				Ok(listener) => listener,
				Err(error) => {
					eprintln!("[movie-box] liaison du serveur embarqué impossible : {error}");
					return Ok(());
				}
			};
			let _ = listener.set_nonblocking(true);
			if let Ok(addr) = listener.local_addr() {
				SERVER_PORT.store(addr.port(), Ordering::Relaxed);
				std::env::set_var("PORT", addr.port().to_string());
			}

			let dist_dir = data_dir.join("web");

			// Le serveur (et l'initialisation des téléchargements, qui lance des
			// tâches tokio) tourne dans le runtime async de Tauri.
			tauri::async_runtime::spawn(async move {
				movie_box_server::init_state();

				match tokio::net::TcpListener::from_std(listener) {
					Ok(listener) => {
						if let Err(error) = movie_box_server::serve(listener, dist_dir).await {
							eprintln!("[movie-box] serveur embarqué arrêté : {error}");
						}
					}
					Err(error) => {
						eprintln!("[movie-box] conversion du listener impossible : {error}");
					}
				}
			});

			Ok(())
		})
		.invoke_handler(tauri::generate_handler![server_origin])
		.run(tauri::generate_context!())
		.expect("erreur au lancement de Movie Box");
}
