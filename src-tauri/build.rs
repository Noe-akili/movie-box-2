fn main() {
	// La clé TMDB est lue depuis le `.env` à la racine du dépôt et intégrée au
	// binaire au moment de la compilation (indispensable sur Android, où aucun
	// fichier `.env` n'existe à l'exécution).
	emit_tmdb_key();

	tauri_build::build()
}

/// Lit `../.env` et expose la clé TMDB via `cargo:rustc-env`.
/// Accepte `TMDB_API_KEY` (prioritaire) ou `VITE_API_KEY`.
fn emit_tmdb_key() {
	println!("cargo:rerun-if-changed=../.env");

	let Ok(contents) = std::fs::read_to_string("../.env") else {
		return;
	};

	let mut tmdb = String::new();
	let mut vite = String::new();

	for line in contents.lines() {
		let line = line.trim();
		if line.is_empty() || line.starts_with('#') {
			continue;
		}
		let Some((name, value)) = line.split_once('=') else {
			continue;
		};
		let value = value.trim().trim_matches('"').trim_matches('\'');

		match name.trim() {
			"TMDB_API_KEY" => tmdb = value.to_string(),
			"VITE_API_KEY" => vite = value.to_string(),
			_ => {}
		}
	}

	let key = if tmdb.is_empty() { vite } else { tmdb };

	if !key.is_empty() {
		println!("cargo:rustc-env=MOVIEBOX_TMDB_API_KEY={key}");
	}
}
