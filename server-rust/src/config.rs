//! Configuration : lit `.env` (racine + dossier du binaire) puis l'environnement shell.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

pub struct DownloadsConfig {
	pub concurrency: usize,
	pub max_bytes: u64,
	pub max_queue: usize,
}

pub struct RateLimitConfig {
	pub window_ms: u64,
	pub max: u32,
}

pub struct Ttl {
	/// Millisecondes.
	pub short: u64,
	pub medium: u64,
	pub long: u64,
}

pub struct Config {
	pub root_dir: PathBuf,
	pub dist_dir: PathBuf,
	pub port: u16,
	/// Port d'écoute du partage de fichiers en Wi‑Fi local.
	pub transfer_port: u16,
	pub tmdb_api_key: String,
	pub tmdb_base_url: String,
	pub tmdb_image_base_url: String,
	pub archive_base_url: String,
	pub commons_api_url: String,
	pub commons_upload_url: String,
	pub watch_region: String,
	pub allowed_origins: Vec<String>,
	pub downloads_dir: PathBuf,
	pub downloads: DownloadsConfig,
	pub rate_limit: RateLimitConfig,
	pub home_deadline_ms: u64,
	pub ttl: Ttl,
	/// API moviebox-internal-api (source de vidéos supplémentaire).
	pub internal_api_url: String,
	pub internal_api_enabled: bool,
	/// NASA Image & Video Library (vidéos du domaine public).
	pub nasa_api_url: String,
	/// Autres catalogues libres (clés facultatives : source masquée sans clé).
	pub europeana_api_url: String,
	pub europeana_api_key: String,
	pub pexels_api_key: String,
	pub pixabay_api_key: String,
	/// Library of Congress : API JSON parfois protégée par Cloudflare.
	pub loc_api_url: String,
	pub loc_api_enabled: bool,
}

fn read_env_file(path: &PathBuf) -> HashMap<String, String> {
	let mut values = HashMap::new();
	let Ok(content) = fs::read_to_string(path) else {
		return values;
	};

	for line in content.split('\n') {
		let trimmed = line.trim();
		if trimmed.is_empty() || trimmed.starts_with('#') {
			continue;
		}
		let Some(separator) = trimmed.find('=') else {
			continue;
		};
		let key = trimmed[..separator].trim();
		let mut value = trimmed[separator + 1..].trim();
		if value.len() >= 2
			&& ((value.starts_with('"') && value.ends_with('"'))
				|| (value.starts_with('\'') && value.ends_with('\'')))
		{
			value = &value[1..value.len() - 1];
		}
		if !key.is_empty() {
			values.insert(key.to_string(), value.to_string());
		}
	}

	values
}

fn to_list(value: &str) -> Vec<String> {
	let list: Vec<String> = value
		.split(',')
		.map(|item| item.trim().to_string())
		.filter(|item| !item.is_empty())
		.collect();

	if list.is_empty() {
		vec!["*".to_string()]
	} else {
		list
	}
}

fn number(env: &HashMap<String, String>, key: &str, default: u64) -> u64 {
	env.get(key)
		.and_then(|value| value.trim().parse::<u64>().ok())
		.unwrap_or(default)
}

fn env_value(
	file_env: &HashMap<String, String>,
	key: &str,
	default: &str,
) -> String {
	env::var(key)
		.ok()
		.filter(|value| !value.is_empty())
		.or_else(|| file_env.get(key).cloned())
		.unwrap_or_else(|| default.to_string())
}

fn from_env() -> Config {
	let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let root_dir = manifest_dir
		.parent()
		.map(|parent| parent.to_path_buf())
		.unwrap_or(manifest_dir.clone());

	// Fichier .env de la racine puis éventuel .env local (priorité au shell).
	let mut file_env = read_env_file(&root_dir.join(".env"));
	for (key, value) in read_env_file(&manifest_dir.join(".env")) {
		file_env.insert(key, value);
	}

	let port = env_value(&file_env, "PORT", "")
		.parse::<u16>()
		.ok()
		.or_else(|| env_value(&file_env, "SERVER_PORT", "").parse::<u16>().ok())
		.unwrap_or(5174);

	let tmdb_api_key = env_value(&file_env, "TMDB_API_KEY", "");
	let tmdb_api_key = if tmdb_api_key.is_empty() {
		env_value(&file_env, "VITE_API_KEY", "")
	} else {
		tmdb_api_key
	};

	let downloads_dir = match env_value(&file_env, "DOWNLOADS_DIR", "") {
		value if value.is_empty() => root_dir.join("server-rust").join("downloads"),
		value => {
			let path = PathBuf::from(&value);
			if path.is_absolute() {
				path
			} else {
				root_dir.join(path)
			}
		}
	};

	let internal_enabled = match env_value(&file_env, "INTERNAL_API_ENABLED", "true")
		.trim()
		.to_ascii_lowercase()
		.as_str()
	{
		"false" | "0" | "no" | "off" => false,
		_ => true,
	};

	Config {
		dist_dir: root_dir.join("dist"),
		port,
		transfer_port: number(&file_env, "TRANSFER_PORT", 8090) as u16,
		tmdb_api_key,
		tmdb_base_url: env_value(
			&file_env,
			"TMDB_BASE_URL",
			"https://api.themoviedb.org/3",
		),
		tmdb_image_base_url: "https://image.tmdb.org/t/p".to_string(),
		archive_base_url: env_value(&file_env, "ARCHIVE_BASE_URL", "https://archive.org"),
		commons_api_url: env_value(
			&file_env,
			"COMMONS_API_URL",
			"https://commons.wikimedia.org",
		),
		commons_upload_url: env_value(
			&file_env,
			"COMMONS_UPLOAD_URL",
			"https://upload.wikimedia.org",
		),
		watch_region: env_value(&file_env, "WATCH_REGION", "FR")
			.to_uppercase()
			.chars()
			.take(2)
			.collect(),
		allowed_origins: to_list(&env_value(&file_env, "ALLOWED_ORIGINS", "*")),
		downloads_dir,
		downloads: DownloadsConfig {
			concurrency: number(&file_env, "DOWNLOAD_CONCURRENCY", 2).clamp(1, 4) as usize,
			max_bytes: number(&file_env, "DOWNLOAD_MAX_BYTES", 8 * 1024 * 1024 * 1024),
			max_queue: number(&file_env, "DOWNLOAD_MAX_QUEUE", 60) as usize,
		},
		rate_limit: RateLimitConfig {
			window_ms: number(&file_env, "RATE_LIMIT_WINDOW_MS", 60_000),
			max: number(&file_env, "RATE_LIMIT_MAX", 240) as u32,
		},
		home_deadline_ms: number(&file_env, "HOME_DEADLINE_MS", 12_000),
		ttl: Ttl {
			short: 5 * 60 * 1000,
			medium: 30 * 60 * 1000,
			long: 6 * 60 * 60 * 1000,
		},
		internal_api_url: env_value(&file_env, "INTERNAL_API_URL", "http://127.0.0.1:8000")
			.trim_end_matches('/')
			.to_string(),
		internal_api_enabled: internal_enabled,
		nasa_api_url: env_value(
			&file_env,
			"NASA_API_URL",
			"https://images-api.nasa.gov",
		)
		.trim_end_matches('/')
		.to_string(),
		europeana_api_url: env_value(
			&file_env,
			"EUROPEANA_API_URL",
			"https://api.europeana.eu",
		)
		.trim_end_matches('/')
		.to_string(),
		europeana_api_key: env_value(&file_env, "EUROPEANA_API_KEY", ""),
		pexels_api_key: env_value(&file_env, "PEXELS_API_KEY", ""),
		pixabay_api_key: env_value(&file_env, "PIXABAY_API_KEY", ""),
		loc_api_url: env_value(&file_env, "LOC_API_URL", "https://www.loc.gov")
			.trim_end_matches('/')
			.to_string(),
		loc_api_enabled: match env_value(&file_env, "LOC_API_ENABLED", "false")
			.trim()
			.to_ascii_lowercase()
			.as_str()
		{
			"true" | "1" | "yes" | "on" => true,
			_ => false,
		},
		root_dir,
	}
}

pub fn config() -> &'static Config {
	static CONFIG: OnceLock<Config> = OnceLock::new();
	CONFIG.get_or_init(from_env)
}

pub fn has_tmdb_key() -> bool {
	!config().tmdb_api_key.is_empty()
}
