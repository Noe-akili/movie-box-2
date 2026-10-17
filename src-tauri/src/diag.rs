//! Journal de diagnostic embarqué.
//!
//! Android ne laisse pas facilement accéder à `logcat` : on conserve donc en
//! mémoire un anneau des derniers évènements (démarrage, paniques, erreurs de
//! l'IPC) et on l'expose au frontend via la commande `backend_diag`. Les mêmes
//! lignes partent sur `stderr` (donc `logcat`, tag `RustStdoutStderr`).

use std::any::Any;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombre de lignes conservées (assez pour un démarrage + quelques requêtes).
const MAX_LINES: usize = 500;

fn buffer() -> &'static Mutex<VecDeque<String>> {
	static LOGS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
	LOGS.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Horodatage relatif simple (millisecondes epoch) : indépendant de la locale.
fn stamp() -> u128 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis())
		.unwrap_or(0)
}

/// Consigne une ligne : à la fois sur `stderr` et dans l'anneau mémoire.
pub fn log(message: impl AsRef<str>) {
	let message = message.as_ref();
	eprintln!("[movie-box] {message}");
	if let Ok(mut logs) = buffer().lock() {
		if logs.len() >= MAX_LINES {
			logs.pop_front();
		}
		logs.push_back(format!("[{}] {message}", stamp()));
	}
}

/// Renvoie l'intégralité du journal (le plus ancien d'abord).
pub fn snapshot() -> String {
	match buffer().lock() {
		Ok(logs) => logs.iter().cloned().collect::<Vec<_>>().join("\n"),
		Err(_) => String::new(),
	}
}

/// Extrait un texte lisible d'une charge utile de panique (`&str`/`String`).
pub fn payload_text(payload: &(dyn Any + Send)) -> String {
	if let Some(text) = payload.downcast_ref::<&str>() {
		(*text).to_string()
	} else if let Some(text) = payload.downcast_ref::<String>() {
		text.clone()
	} else {
		"panique (charge utile non textuelle)".to_string()
	}
}

/// Installe un crochet qui journalise chaque panique (message + localisation)
/// avant de déléguer au crochet par défaut (trace sur `stderr`/logcat).
pub fn install_panic_hook() {
	let previous = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		let location = info
			.location()
			.map(|location| format!("{}:{}:{}", location.file(), location.line(), location.column()))
			.unwrap_or_else(|| "localisation inconnue".to_string());
		log(format!(
			"PANIQUE à {location} — {}",
			payload_text(info.payload())
		));
		previous(info);
	}));
}
