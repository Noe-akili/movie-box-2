//! Point d'entrée du serveur autonome Movie Box.
//!
//! Toute la logique vit dans la bibliothèque `movie_box_server` (partagée avec
//! l'app Tauri Android).

#[tokio::main]
async fn main() {
	movie_box_server::run_standalone().await;
}
