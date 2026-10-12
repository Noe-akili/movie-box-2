//! Téléchargements du backend : file d'attente, progression, reprise et re-lecture locale.
//!
//! Reproduit `services/downloads.js` :
//! - manifeste JSON : l'état survit à un redémarrage du serveur ;
//! - fichier `.part` : la reprise repart de l'octet déjà reçu (HTTP Range) ;
//! - seules les sources autorisées (Internet Archive / Wikimedia) sont acceptées.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Write;

use crate::config::config;
use crate::error::{ApiError, Result};
use crate::services::free::{free_remote_url, free_source_label, get_free_file};
use sha1::{Digest, Sha1};

const PARTIAL_SUFFIX: &str = ".part";

/// Formats que le navigateur lit directement (les autres restent enregistrables).
const BROWSER_PLAYABLE: [&str; 4] = [".mp4", ".m4v", ".webm", ".ogv"];

fn mime_for(file_name: &str) -> &'static str {
	match extension_of(file_name).as_str() {
		".mp4" => "video/mp4",
		".m4v" => "video/x-m4v",
		".webm" => "video/webm",
		".ogv" => "video/ogg",
		".mkv" => "video/x-matroska",
		".avi" => "video/x-msvideo",
		".mov" => "video/quicktime",
		".mpg" | ".mpeg" => "video/mpeg",
		_ => "application/octet-stream",
	}
}

pub fn extension_of(file_name: &str) -> String {
	match file_name.rfind('.') {
		Some(index) if file_name[index..].len() <= 8 => file_name[index..].to_ascii_lowercase(),
		_ => String::new(),
	}
}

pub fn is_playable_file(file_name: &str) -> bool {
	let extension = extension_of(file_name);
	BROWSER_PLAYABLE.contains(&extension.as_str())
}

pub fn mime_for_file(file_name: &str) -> &'static str {
	mime_for(file_name)
}

fn now_ms() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as i64)
		.unwrap_or(0)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEntry {
	pub id: String,
	pub source_id: String,
	pub file_name: String,
	#[serde(default)]
	pub extension: String,
	pub title: String,
	pub kind: String,
	#[serde(default)]
	pub series_title: Option<String>,
	#[serde(default)]
	pub season_number: Option<i64>,
	#[serde(default)]
	pub episode_number: Option<i64>,
	#[serde(default)]
	pub episode_name: Option<String>,
	#[serde(default)]
	pub label: Option<String>,
	#[serde(default)]
	pub poster: Option<String>,
	#[serde(default)]
	pub backdrop: Option<String>,
	#[serde(default)]
	pub quality: Option<String>,
	#[serde(default)]
	pub format: Option<String>,
	/// URL distante résolue au moment de la création (sources à URL signée
	/// qui expire, comme MovieBox). Prioritaire sur `free_remote_url`.
	#[serde(default)]
	pub remote_url: Option<String>,
	pub bytes_total: u64,
	pub bytes_received: u64,
	pub speed_bps: u64,
	pub status: String,
	#[serde(default)]
	pub error: Option<String>,
	pub created_at: i64,
	#[serde(default)]
	pub started_at: Option<i64>,
	pub updated_at: i64,
	#[serde(default)]
	pub completed_at: Option<i64>,
	#[serde(default)]
	pub removed: bool,
}

impl DownloadEntry {
	pub fn progress(&self) -> f64 {
		if self.bytes_total > 0 {
			let raw = (self.bytes_received as f64 / self.bytes_total as f64) * 100.0;
			(raw.round() * 10.0 / 10.0).min(100.0)
		} else if self.status == "completed" {
			100.0
		} else {
			0.0
		}
	}

	pub fn eta_seconds(&self) -> Option<i64> {
		if self.status != "downloading"
			|| self.speed_bps == 0
			|| self.bytes_total <= self.bytes_received
		{
			return None;
		}
		let remaining = self.bytes_total - self.bytes_received;
		Some((remaining / self.speed_bps).max(1) as i64)
	}
}

fn downloads_dir() -> PathBuf {
	config().downloads_dir.clone()
}

fn manifest_file() -> PathBuf {
	downloads_dir().join("manifest.json")
}

pub fn file_path_of(entry: &DownloadEntry) -> PathBuf {
	let extension = if entry.extension.is_empty() {
		extension_of(&entry.file_name)
	} else {
		entry.extension.clone()
	};
	downloads_dir().join(format!("{}{}", entry.id, extension))
}

fn partial_path_of(entry: &DownloadEntry) -> PathBuf {
	PathBuf::from(format!("{}{}", file_path_of(entry).display(), PARTIAL_SUFFIX))
}

fn clean_text(value: Option<&str>, max: usize) -> Option<String> {
	let text = value?.split_whitespace().collect::<Vec<_>>().join(" ");
	if text.is_empty() {
		None
	} else {
		Some(text.chars().take(max).collect())
	}
}

fn clean_image(value: Option<&Value>) -> Option<String> {
	let url = value?.as_str()?.trim().to_string();
	if url.starts_with("http://") || url.starts_with("https://") {
		Some(url.chars().take(500).collect())
	} else {
		None
	}
}

fn clean_number(value: Option<&Value>) -> Option<i64> {
	match value? {
		Value::Number(number) => number.as_i64().filter(|value| *value >= 0),
		Value::String(text) => text.parse::<i64>().ok().filter(|value| *value >= 0),
		_ => None,
	}
}

/// Identifiant stable : identifiant d'archive normalisé + empreinte du nom de fichier.
fn build_id(identifier: &str, file_name: &str) -> String {
	let slug: String = identifier
		.to_ascii_lowercase()
		.chars()
		.map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
		.collect::<String>()
		.split('-')
		.filter(|part| !part.is_empty())
		.collect::<Vec<_>>()
		.join("-")
		.chars()
		.take(48)
		.collect::<String>();
	let hash = Sha1::digest(file_name.as_bytes())
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect::<String>();
	format!(
		"{}-{}",
		if slug.is_empty() { "media" } else { &slug },
		&hash[..10]
	)
}

fn urlencode_id(value: &str) -> String {
	crate::services::providers::urlencode(value)
}

/// Vue publique d'un téléchargement (sans chemin serveur) prête pour l'interface.
pub fn public_entry(entry: &DownloadEntry) -> Value {
	json!({
		"id": entry.id,
		"sourceId": entry.source_id,
		"fileName": entry.file_name,
		"title": entry.title,
		"kind": entry.kind,
		"seriesTitle": entry.series_title,
		"seasonNumber": entry.season_number,
		"episodeNumber": entry.episode_number,
		"episodeName": entry.episode_name,
		"label": entry.label,
		"poster": entry.poster,
		"backdrop": entry.backdrop,
		"quality": entry.quality,
		"format": entry.format,
		"status": entry.status,
		"error": entry.error,
		"bytesTotal": entry.bytes_total,
		"bytesReceived": entry.bytes_received,
		"progress": entry.progress(),
		"speedBps": entry.speed_bps,
		"etaSeconds": entry.eta_seconds(),
		"playable": entry.status == "completed" && is_playable_file(&entry.file_name),
		"streamUrl": format!("/api/downloads/{}/stream", urlencode_id(&entry.id)),
		"fileUrl": format!("/api/downloads/{}/file", urlencode_id(&entry.id)),
		"createdAt": entry.created_at,
		"startedAt": entry.started_at,
		"updatedAt": entry.updated_at,
		"completedAt": entry.completed_at,
	})
}

pub struct Manager {
	entries: Mutex<HashMap<String, DownloadEntry>>,
	cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
	running: AtomicUsize,
	persist_scheduled: AtomicBool,
}

static MANAGER: LazyLock<Manager> = LazyLock::new(|| Manager {
	entries: Mutex::new(HashMap::new()),
	cancellations: Mutex::new(HashMap::new()),
	running: AtomicUsize::new(0),
	persist_scheduled: AtomicBool::new(false),
});

pub fn manager() -> &'static Manager {
	&MANAGER
}

impl Manager {
	fn write_manifest_now(&self) {
		let entries: Vec<DownloadEntry> = {
			let guard = self.entries.lock().unwrap();
			guard.values().cloned().collect()
		};
		let payload = json!({ "version": 1, "entries": entries });
		let Ok(serialized) = serde_json::to_string(&payload) else {
			return;
		};

		let file = manifest_file();
		let tmp = file.with_extension("json.tmp");
		if std::fs::write(&tmp, serialized).is_ok() {
			let _ = std::fs::rename(&tmp, &file);
		} else {
			eprintln!("[movie-box] manifeste de téléchargements non enregistré");
		}
	}

	/// Écrit le manifeste (au plus une fois par seconde pendant un transfert).
	pub fn persist(&self, immediate: bool) {
		if immediate {
			self.write_manifest_now();
			return;
		}
		if self
			.persist_scheduled
			.swap(true, std::sync::atomic::Ordering::SeqCst)
		{
			return;
		}
		tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(900)).await;
			manager()
				.persist_scheduled
				.store(false, std::sync::atomic::Ordering::SeqCst);
			manager().write_manifest_now();
		});
	}

	/// Recharge le manifeste au démarrage (les transferts inachevés repartent en file).
	pub fn restore(&self) {
		let _ = std::fs::create_dir_all(downloads_dir());
		let Ok(content) = std::fs::read_to_string(manifest_file()) else {
			return;
		};
		let Ok(payload) = serde_json::from_str::<Value>(&content) else {
			return;
		};
		let Some(stored) = payload.get("entries").and_then(|value| value.as_array()) else {
			return;
		};

		let mut restored = 0;
		{
			let mut entries = self.entries.lock().unwrap();
			for raw in stored {
				let Ok(mut entry) = serde_json::from_value::<DownloadEntry>(raw.clone()) else {
					continue;
				};
				if entry.id.is_empty() || entry.file_name.is_empty() || entry.source_id.is_empty() {
					continue;
				}
				entry.extension = extension_of(&entry.file_name);
				entry.title = clean_text(Some(&entry.title), 200)
					.unwrap_or_else(|| entry.file_name.clone());
				entry.removed = false;
				entry.speed_bps = 0;
				entry.error = None;
				if entry.status == "queued" || entry.status == "downloading" {
					entry.status = "queued".to_string();
				}
				if entry.status != "completed" {
					entry.bytes_received = std::fs::metadata(partial_path_of(&entry))
						.map(|meta| meta.len())
						.unwrap_or(0);
				}
				restored += 1;
				entries.insert(entry.id.clone(), entry);
			}
		}

		if restored > 0 {
			let queued = self.count_status("queued");
			if queued > 0 {
				eprintln!("[movie-box] {queued} téléchargement(s) repris après redémarrage");
			}
		}
	}

	pub fn count_status(&self, status: &str) -> usize {
		self.entries
			.lock()
			.unwrap()
			.values()
			.filter(|entry| entry.status == status)
			.count()
	}

	pub fn list(&self) -> Vec<Value> {
		let mut entries: Vec<DownloadEntry> =
			self.entries.lock().unwrap().values().cloned().collect();
		entries.sort_by(|a, b| {
			let rank = |status: &str| match status {
				"queued" | "downloading" => 0,
				"completed" => 2,
				_ => 1,
			};
			rank(&a.status)
				.cmp(&rank(&b.status))
				.then(b.created_at.cmp(&a.created_at))
		});
		entries.iter().map(public_entry).collect()
	}

	pub fn entry(&self, id: &str) -> Result<DownloadEntry> {
		self.entries
			.lock()
			.unwrap()
			.get(id)
			.cloned()
			.ok_or_else(|| ApiError::not_found("Téléchargement introuvable"))
	}

	pub fn summary(&self) -> Value {
		let items: Vec<DownloadEntry> = self.entries.lock().unwrap().values().cloned().collect();
		let count = |status: &str| {
			items
				.iter()
				.filter(|entry| entry.status == status)
				.count() as i64
		};
		let active: Vec<&DownloadEntry> = items
			.iter()
			.filter(|entry| entry.status == "queued" || entry.status == "downloading")
			.collect();
		let completed: Vec<&DownloadEntry> = items
			.iter()
			.filter(|entry| entry.status == "completed")
			.collect();

		json!({
			"total": items.len(),
			"completed": completed.len(),
			"active": active.len(),
			"queued": count("queued"),
			"errors": count("error"),
			"paused": count("paused") + count("canceled"),
			"completedBytes": completed.iter().map(|entry| entry.bytes_total).sum::<u64>(),
			"activeBytes": active.iter().map(|entry| entry.bytes_received).sum::<u64>(),
			"maxBytes": config().downloads.max_bytes,
			"maxQueue": config().downloads.max_queue,
			"concurrency": config().downloads.concurrency,
			"freeBytes": free_disk_bytes(&downloads_dir()),
		})
	}
}

fn free_disk_bytes(dir: &Path) -> Option<u64> {
	#[cfg(target_os = "linux")]
	{
		use std::os::unix::ffi::OsStrExt;
		let c_path = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
		unsafe {
			let mut stats: libc::statfs = std::mem::zeroed();
			if libc::statfs(c_path.as_ptr(), &mut stats) != 0 {
				return None;
			}
			Some(stats.f_bavail as u64 * stats.f_bsize as u64)
		}
	}
	#[cfg(not(target_os = "linux"))]
	{
		let _ = dir;
		None
	}
}

impl Manager {
	/// Crée (ou renvoie s'il existe déjà) un téléchargement après validation du fichier.
	pub async fn create(&self, payload: &Value) -> Result<Value> {
		let source_id = clean_text(
			payload
				.get("sourceId")
				.or_else(|| payload.get("identifier"))
				.and_then(|value| value.as_str()),
			240,
		)
		.ok_or_else(|| ApiError::bad_request("Identifiant source requis"))?;
		let file_name = clean_text(
			payload
				.get("fileName")
				.or_else(|| payload.get("file"))
				.and_then(|value| value.as_str()),
			200,
		)
		.ok_or_else(|| ApiError::bad_request("Nom de fichier requis"))?;

		let file = get_free_file(&source_id, &file_name).await?;
		let identifier = file
			.get("identifier")
			.and_then(|value| value.as_str())
			.unwrap_or(&source_id)
			.to_string();
		let name = file
			.get("name")
			.and_then(|value| value.as_str())
			.unwrap_or(&file_name)
			.to_string();
		let id = build_id(&identifier, &name);

		if let Some(existing) = self.entries.lock().unwrap().get(&id) {
			return Ok(public_entry(existing));
		}

		if self.entries.lock().unwrap().len() >= config().downloads.max_queue {
			return Err(ApiError::new(
				429,
				format!(
					"File d'attente pleine ({} fichiers) : supprime un téléchargement avant d'en lancer un autre",
					config().downloads.max_queue
				),
			));
		}

		let size = file
			.get("size")
			.and_then(|value| value.as_u64())
			.or_else(|| clean_number(payload.get("size")).and_then(|value| u64::try_from(value).ok()))
			.unwrap_or(0);
		if size > config().downloads.max_bytes {
			return Err(ApiError::new(
				413,
				format!(
					"Fichier trop volumineux ({:.1} Go) : la limite du serveur est de {:.1} Go",
					size as f64 / 1024_f64.powi(3),
					config().downloads.max_bytes as f64 / 1024_f64.powi(3)
				),
			));
		}

				let kind = if payload.get("kind").and_then(|value| value.as_str()) == Some("episode") {
			"episode"
		} else {
			"movie"
		};
		let now = now_ms();

		let entry = DownloadEntry {
			id: id.clone(),
			source_id: identifier,
			file_name: name.clone(),
			extension: extension_of(&name),
			title: clean_text(
				payload.get("title").and_then(|value| value.as_str()),
				200,
			)
			.unwrap_or_else(|| name.clone()),
			kind: kind.to_string(),
			series_title: if kind == "episode" {
				clean_text(
					payload.get("seriesTitle").and_then(|value| value.as_str()),
					200,
				)
			} else {
				None
			},
			season_number: if kind == "episode" {
				clean_number(payload.get("seasonNumber"))
			} else {
				None
			},
			episode_number: if kind == "episode" {
				clean_number(payload.get("episodeNumber"))
			} else {
				None
			},
			episode_name: if kind == "episode" {
				clean_text(
					payload.get("episodeName").and_then(|value| value.as_str()),
					200,
				)
			} else {
				None
			},
			label: if kind == "episode" {
				clean_text(payload.get("label").and_then(|value| value.as_str()), 40)
			} else {
				None
			},
			poster: clean_image(payload.get("poster")),
			backdrop: clean_image(payload.get("backdrop")),
			quality: clean_text(
				payload.get("quality").and_then(|value| value.as_str()),
				20,
			),
			format: clean_text(
				payload.get("format").and_then(|value| value.as_str()),
				40,
			)
			.or_else(|| {
				file.get("format")
					.and_then(|value| value.as_str())
					.map(|value| value.to_string())
			}),
			remote_url: file
				.get("url")
				.and_then(|value| value.as_str())
				.filter(|value| !value.is_empty())
				.map(|value| value.to_string()),
			bytes_total: size,
			bytes_received: 0,
			speed_bps: 0,
			status: "queued".to_string(),
			error: None,
			created_at: now,
			started_at: None,
			updated_at: now,
			completed_at: None,
			removed: false,
		};

		let view = public_entry(&entry);
		self.entries.lock().unwrap().insert(id, entry);
		self.persist(true);
		self.enqueue();

		Ok(view)
	}

	/// Relance un téléchargement en erreur, interrompu ou annulé (reprise à l'octet reçu).
	pub fn retry(&self, id: &str) -> Result<Value> {
		let mut entries = self.entries.lock().unwrap();
		let entry = entries
			.get_mut(id)
			.ok_or_else(|| ApiError::not_found("Téléchargement introuvable"))?;
		if entry.status == "downloading" {
			return Ok(public_entry(entry));
		}

		entry.bytes_received = std::fs::metadata(partial_path_of(entry))
			.map(|meta| meta.len())
			.unwrap_or(0);
		entry.speed_bps = 0;
		entry.error = None;
		entry.status = "queued".to_string();
		entry.updated_at = now_ms();
		let view = public_entry(entry);
		drop(entries);

		self.persist(true);
		self.enqueue();
		Ok(view)
	}

	/// Stoppe un transfert en cours en conservant la partie déjà reçue.
	pub fn cancel(&self, id: &str) -> Result<Value> {
		let mut entries = self.entries.lock().unwrap();
		let entry = entries
			.get_mut(id)
			.ok_or_else(|| ApiError::not_found("Téléchargement introuvable"))?;

		entry.status = "canceled".to_string();
		entry.updated_at = now_ms();
		let view = public_entry(entry);
		drop(entries);

		if let Some(flag) = self.cancellations.lock().unwrap().get(id) {
			flag.store(true, std::sync::atomic::Ordering::SeqCst);
		}

		self.persist(true);
		Ok(view)
	}

	/// Supprime un téléchargement et ses fichiers sur le disque.
	pub fn remove(&self, id: &str) -> Result<Value> {
		let entry = self.entry(id)?;
		if let Some(flag) = self.cancellations.lock().unwrap().get(id) {
			flag.store(true, std::sync::atomic::Ordering::SeqCst);
		}

		let mut entries = self.entries.lock().unwrap();
		let mut entry = entry;
		entry.removed = true;
		entries.remove(id);
		drop(entries);

		let final_path = file_path_of(&entry);
		let partial = partial_path_of(&entry);
		let _ = std::fs::remove_file(&final_path);
		let _ = std::fs::remove_file(&partial);

		// Le transfert s'arrête bientôt : on repasse une seconde après pour nettoyer.
		tokio::spawn(async move {
			tokio::time::sleep(Duration::from_secs(1)).await;
			let _ = std::fs::remove_file(&final_path);
			let _ = std::fs::remove_file(&partial);
		});

		self.persist(true);
		Ok(json!({ "id": id, "removed": true }))
	}

	/// Un slot libre = on lance le prochain téléchargement en attente.
	pub fn enqueue(&self) {
		loop {
			if self.running.load(std::sync::atomic::Ordering::SeqCst)
				>= config().downloads.concurrency
			{
				return;
			}

			let next = {
				let mut entries = self.entries.lock().unwrap();
				entries
					.values_mut()
					.find(|entry| entry.status == "queued" && !entry.removed)
					.map(|entry| {
						entry.status = "downloading".to_string();
						entry.started_at = entry.started_at.or(Some(now_ms()));
						entry.updated_at = now_ms();
						entry.error = None;
						entry.clone()
					})
			};

			let Some(entry) = next else {
				return;
			};

			self.running.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
			self.persist(true);

			let flag = Arc::new(AtomicBool::new(false));
			self.cancellations
				.lock()
				.unwrap()
				.insert(entry.id.clone(), flag.clone());

			tokio::spawn(async move {
				run(&entry.id, flag).await;

				// Fin de transfert : on libère la place pour le suivant.
				let manager = manager();
				manager.cancellations.lock().unwrap().remove(&entry.id);
				manager.running.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
				manager.persist(true);
				manager.enqueue();
			});
		}
	}

	/// Prépare le dossier de stockage, restaure le manifeste et relance la file.
	pub fn init(&self) {
		let _ = std::fs::create_dir_all(downloads_dir());
		self.restore();
		self.enqueue();
	}
}

fn is_cancelled(flag: &AtomicBool) -> bool {
	flag.load(std::sync::atomic::Ordering::SeqCst)
}

async fn run(id: &str, flag: Arc<AtomicBool>) {
	if let Err(error) = run_inner(id, &flag).await {
		if let Ok(mut entries) = manager().entries.lock() {
			if let Some(entry) = entries.get_mut(id) {
				if !entry.removed && entry.status != "canceled" {
					entry.status = "error".to_string();
					entry.error = Some(error.message.clone());
				}
				entry.speed_bps = 0;
				entry.updated_at = now_ms();
			}
		}
		eprintln!("[movie-box] téléchargement interrompu : {}", error.message);
	}
}

/// Transfert d'un fichier avec suivi de progression, reprise (HTTP Range) et annulation.
async fn run_inner(id: &str, flag: &Arc<AtomicBool>) -> Result<()> {
	let manager = manager();
	let entry = manager.entry(id)?;
	let final_path = file_path_of(&entry);
	let partial = partial_path_of(&entry);

	// Reprise : on repart de l'octet déjà écrit.
	let mut start_at = std::fs::metadata(&partial)
		.map(|meta| meta.len())
		.unwrap_or(0);
	if entry.bytes_total > 0 && start_at >= entry.bytes_total {
		start_at = 0;
	}

	let remote_url = entry
		.remote_url
		.clone()
		.or_else(|| free_remote_url(&entry.source_id, &entry.file_name))
		.ok_or_else(|| ApiError::bad_request("Fichier distant introuvable pour cette source"))?;

	let mut request = crate::http::client()
		.get(&remote_url)
		.header("user-agent", "movie-box/1.0")
		// Le CDN vidéo MovieBox applique une protection anti-hotlink qui
		// renvoie 429 sauf pour certains referers autorisés.
		.header("referer", "https://movieboxapp.in/");
	if start_at > 0 {
		request = request.header("range", format!("bytes={start_at}-"));
	}

	let response = request
		.send()
		.await
		.map_err(|_| ApiError::upstream(502, "Connexion au serveur distant impossible"))?;
	let status = response.status();
	if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
		return Err(ApiError::upstream(
			502,
			format!(
				"{} a refusé le téléchargement ({})",
				free_source_label(&entry.source_id),
				status.as_u16()
			),
		));
	}

	let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
	if !resumed && start_at > 0 {
		// Le serveur ignore la reprise : on repart du début.
		let _ = std::fs::remove_file(&partial);
		start_at = 0;
	}

	let content_length = response
		.headers()
		.get("content-length")
		.and_then(|value| value.to_str().ok())
		.and_then(|value| value.parse::<u64>().ok())
		.unwrap_or(0);
	let total = if resumed {
		start_at + content_length
	} else if content_length > 0 {
		content_length
	} else {
		entry.bytes_total
	};
	if total > config().downloads.max_bytes {
		return Err(ApiError::new(
			413,
			"Fichier trop volumineux pour le stockage du serveur",
		));
	}

	{
		let mut entries = manager.entries.lock().unwrap();
		if let Some(current) = entries.get_mut(id) {
			current.bytes_total = if total > 0 { total } else { current.bytes_total };
			current.bytes_received = start_at;
			current.updated_at = now_ms();
		}
	}

	if let Err(error) = std::fs::create_dir_all(downloads_dir()) {
		return Err(ApiError::upstream(
			500,
			format!("Dossier de téléchargement indisponible : {error}"),
		));
	}

	let mut options = std::fs::OpenOptions::new();
	options.create(true).write(true);
	if start_at > 0 {
		options.append(true);
	} else {
		options.truncate(true);
	}
	let mut file = options
		.open(&partial)
		.map_err(|error| ApiError::upstream(500, format!("Écriture impossible : {error}")))?;

	use futures::StreamExt;
	let mut stream = response.bytes_stream();
	let mut received = start_at;
	let mut last_bytes = start_at;
	let mut last_tick = std::time::Instant::now();

	while let Some(chunk) = stream.next().await {
		if is_cancelled(flag) {
			return Ok(());
		}
		let chunk = chunk
			.map_err(|_| ApiError::upstream(502, "Connexion interrompue pendant le transfert"))?;
		file.write_all(&chunk)
			.map_err(|error| ApiError::upstream(500, format!("Écriture impossible : {error}")))?;
		received += chunk.len() as u64;

		{
			let mut entries = manager.entries.lock().unwrap();
			if let Some(current) = entries.get_mut(id) {
				current.bytes_received = received;
			}
		}

		let now = std::time::Instant::now();
		let elapsed = now.duration_since(last_tick);
		if elapsed.as_millis() >= 1_000 {
			let instant = (received - last_bytes) as f64 * 1_000.0 / elapsed.as_millis() as f64;
			{
				let mut entries = manager.entries.lock().unwrap();
				if let Some(current) = entries.get_mut(id) {
					current.speed_bps = if current.speed_bps > 0 {
						(current.speed_bps as f64 * 0.6 + instant * 0.4) as u64
					} else {
						instant as u64
					};
					current.updated_at = now_ms();
				}
			}
			manager.persist(false);
			last_bytes = received;
			last_tick = now;
		}
	}

	drop(file);

	if is_cancelled(flag) {
		return Ok(());
	}

	let final_size = std::fs::metadata(&partial).map(|meta| meta.len()).unwrap_or(0);
	let expected = manager.entry(id)?.bytes_total;
	if expected > 0 && final_size < expected {
		return Err(ApiError::upstream(
			502,
			"Téléchargement incomplet (connexion interrompue)",
		));
	}

	let _ = std::fs::remove_file(&final_path);
	std::fs::rename(&partial, &final_path)
		.map_err(|error| ApiError::upstream(500, format!("Finalisation impossible : {error}")))?;

	let mut entries = manager.entries.lock().unwrap();
	if let Some(current) = entries.get_mut(id) {
		current.bytes_total = if current.bytes_total > 0 {
			current.bytes_total
		} else {
			final_size
		};
		current.bytes_received = final_size;
		current.speed_bps = 0;
		current.status = "completed".to_string();
		current.error = None;
		current.completed_at = Some(now_ms());
		current.updated_at = current.completed_at.unwrap();
	}

	Ok(())
}
