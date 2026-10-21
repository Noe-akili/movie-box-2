//! Partage de fichiers en Wi‑Fi local : une « salle » hébergée par l'appareil.
//!
//! L'appareil ouvre un serveur HTTP sur le réseau local (`0.0.0.0:<port>`). Tous
//! les appareils connectés au même Wi‑Fi ouvrent l'URL ou scannent le QR code et
//! peuvent alors télécharger les fichiers partagés **et en envoyer** : la salle
//! est commune (plusieurs téléphones peuvent s'échanger des fichiers en même temps).
//!
//! Le serveur public est isolé de l'API : il n'expose qu'une page, la liste des
//! fichiers, le téléchargement (avec `Range`) et l'envoi (multipart en flux).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, Request};
use axum::http::header;
use axum::response::{Html, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::config::config;
use crate::error::{ApiError, Result};
use crate::services::downloads::{
	extension_of, file_path_of, is_playable_file, manager, mime_for_name, serve_path,
};

/// Fichier reçu depuis un autre appareil et conservé sur le téléphone.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomFile {
	/// Identifiant public (`up_…`).
	id: String,
	name: String,
	size: u64,
	mime: String,
	#[serde(default)]
	uploader: String,
	/// Identifiant de l'appareil expéditeur : permet de lui masquer ses propres
	/// envois (il n'a pas besoin de re-télécharger ce qu'il vient de partager).
	#[serde(default)]
	uploader_id: String,
	added_at: i64,
	/// Nom du fichier sur le disque, dans le dossier de partage.
	stored_name: String,
}

/// Partage en cours d'écoute.
struct Active {
	token: String,
	port: u16,
	ip: Option<IpAddr>,
	shutdown: Option<oneshot::Sender<()>>,
}

impl Active {
	fn url(&self) -> String {
		let host = self
			.ip
			.map(|ip| ip.to_string())
			.unwrap_or_else(|| "127.0.0.1".to_string());
		format!("http://{host}:{}/{}/", self.port, self.token)
	}
}

/// Fichiers reçus (persistés) ; chargés à la première lecture.
static RECEIVED: LazyLock<Mutex<Vec<RoomFile>>> = LazyLock::new(|| Mutex::new(load_received()));
/// Identifiants des téléchargements du téléphone exposés dans la salle.
static SHARED: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));
/// Partage actif (un seul à la fois).
static ACTIVE: LazyLock<Mutex<Option<Active>>> = LazyLock::new(|| Mutex::new(None));
/// Vrai tant que la salle expose automatiquement tous les téléchargements terminés.
static SHARE_ALL: AtomicBool = AtomicBool::new(true);
/// Salle distante rejointe depuis l'app (l'appareil devient simple participant).
static REMOTE: LazyLock<Mutex<Option<RemoteRoom>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Clone)]
struct RemoteRoom {
	base: String,
	client_id: String,
}

/// Identifiant stable de cet appareil dans les salles rejointes.
fn app_client_id() -> String {
	static ID: LazyLock<String> = LazyLock::new(|| format!("app-{}", make_token()));
	ID.clone()
}

fn transfer_dir() -> PathBuf {
	config().downloads_dir.join("transfer")
}

fn now_ms() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_millis() as i64)
		.unwrap_or(0)
}

fn hex_id(salt: &[u8]) -> String {
	use sha1::{Digest, Sha1};
	static COUNTER: AtomicU64 = AtomicU64::new(0);

	let mut hasher = Sha1::new();
	let nanos = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_nanos())
		.unwrap_or(0);
	hasher.update(nanos.to_le_bytes());
	hasher.update(std::process::id().to_le_bytes());
	hasher.update(COUNTER.fetch_add(1, Ordering::SeqCst).to_le_bytes());
	hasher.update(salt);
	hasher
		.finalize()
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect()
}

fn make_token() -> String {
	hex_id(b"movie-box-room").chars().take(16).collect()
}

fn make_file_id() -> String {
	format!("up_{}", hex_id(b"movie-box-file").chars().take(12).collect::<String>())
}

/// Nom de fichier sûr : retire le chemin, les caractères interdits et les points initiaux.
fn safe_file_name(raw: &str) -> String {
	let base = raw
		.rsplit(|ch| ch == '/' || ch == '\\')
		.next()
		.unwrap_or(raw)
		.trim();
	let cleaned: String = base
		.chars()
		.map(|ch| {
			if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
				'_'
			} else {
				ch
			}
		})
		.collect();
	let trimmed = cleaned.trim_matches(|ch: char| ch == '.' || ch == ' ');
	if trimmed.is_empty() {
		"fichier".to_string()
	} else {
		trimmed.chars().take(160).collect()
	}
}

/// Adresse IPv4 locale (celle de l'interface Wi‑Fi) via la route par défaut.
fn primary_ip() -> Option<IpAddr> {
	let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
	socket.connect((Ipv4Addr::new(1, 1, 1, 1), 80)).ok()?;
	socket.local_addr().ok().map(|addr| addr.ip())
}

fn load_received() -> Vec<RoomFile> {
	let Ok(content) = std::fs::read_to_string(transfer_dir().join("manifest.json")) else {
		return Vec::new();
	};
	let Ok(payload) = serde_json::from_str::<Value>(&content) else {
		return Vec::new();
	};
	payload
		.get("files")
		.and_then(|value| value.as_array())
		.map(|items| {
			items
				.iter()
				.filter_map(|item| serde_json::from_value::<RoomFile>(item.clone()).ok())
				.collect()
		})
		.unwrap_or_default()
}

fn persist_received(files: &[RoomFile]) {
	let dir = transfer_dir();
	let _ = std::fs::create_dir_all(&dir);
	let Ok(text) = serde_json::to_string(&json!({ "version": 1, "files": files })) else {
		return;
	};
	let file = dir.join("manifest.json");
	let tmp = dir.join("manifest.json.tmp");
	if std::fs::write(&tmp, text).is_ok() {
		let _ = std::fs::rename(&tmp, &file);
	}
}

fn received_room_file(file: &RoomFile) -> Value {
	json!({
		"id": file.id,
		"name": file.name,
		"size": file.size,
		"mime": file.mime,
		"kind": "upload",
		"playable": is_playable_file(&file.name),
		"uploader": file.uploader,
		"addedAt": file.added_at,
		"downloadPath": format!("file/{}", file.id),
		// Route locale de l'API (lecture/enregistrement depuis l'application).
		"streamPath": format!("/api/transfer/received/{}/file", file.id),
	})
}

fn shared_room_file(id: &str) -> Option<Value> {
	let entry = manager().entry(id).ok()?;
	if entry.status != "completed" {
		return None;
	}
	Some(json!({
		"id": format!("dl_{}", entry.id),
		"name": entry.file_name,
		"size": entry.bytes_total,
		"mime": mime_for_name(&entry.file_name),
		"kind": "download",
		"playable": is_playable_file(&entry.file_name),
		"uploader": "",
		"addedAt": entry.completed_at.unwrap_or(entry.created_at),
		"downloadPath": format!("file/dl_{}", entry.id),
	}))
}

/// Liste des fichiers visibles dans la salle (téléchargements partagés + reçus).
///
/// `exclude_client` retire les fichiers envoyés par l'appareil demandeur : chacun
/// voit ce qu'il peut récupérer, mais pas ce qu'il vient lui-même de partager.
pub fn visible_files(exclude_client: Option<&str>) -> Vec<Value> {
	let mut out = Vec::new();
	for id in shared_ids() {
		if let Some(item) = shared_room_file(&id) {
			out.push(item);
		}
	}

	let exclude = exclude_client.map(str::trim).filter(|value| !value.is_empty());
	for file in RECEIVED.lock().unwrap().iter() {
		if let Some(client) = exclude {
			if !file.uploader_id.is_empty() && file.uploader_id == client {
				continue;
			}
		}
		out.push(received_room_file(file));
	}
	out
}

fn resolved_path(id: &str) -> Result<(PathBuf, String)> {
	if let Some(download_id) = id.strip_prefix("dl_") {
		let entry = manager().entry(download_id)?;
		if entry.status != "completed" {
			return Err(ApiError::new(409, "Ce fichier n'est pas disponible"));
		}
		return Ok((file_path_of(&entry), entry.file_name));
	}

	let files = RECEIVED.lock().unwrap();
	let file = files
		.iter()
		.find(|file| file.id == id)
		.ok_or_else(|| ApiError::not_found("Fichier introuvable"))?;
	Ok((transfer_dir().join(&file.stored_name), file.name.clone()))
}

/// État du partage pour l'interface (écran « Transferts »).
pub fn status() -> Value {
	let received: Vec<Value> = RECEIVED
		.lock()
		.unwrap()
		.iter()
		.map(received_room_file)
		.collect();
	let shared = shared_ids();
	let shared_all = SHARE_ALL.load(Ordering::SeqCst);
	let remote = REMOTE
		.lock()
		.unwrap()
		.as_ref()
		.map(|room| json!({ "url": room.base }));

	match ACTIVE.lock().unwrap().as_ref() {
		Some(active) => json!({
			"running": true,
			"token": active.token,
			"port": active.port,
			"url": active.url(),
			"shared": shared,
			"sharedAll": shared_all,
			"received": received,
			"remote": remote,
			"files": visible_files(None),
		}),
		None => json!({
			"running": false,
			"port": Value::Null,
			"url": Value::Null,
			"shared": shared,
			"sharedAll": shared_all,
			"received": received,
			"remote": remote,
			"files": [],
		}),
	}
}

fn completed_download_ids() -> Vec<String> {
	manager()
		.list()
		.iter()
		.filter(|item| item.get("status").and_then(|value| value.as_str()) == Some("completed"))
		.filter_map(|item| item.get("id").and_then(|value| value.as_str()).map(String::from))
		.collect()
}

/// Téléchargements exposés : soit tous les fichiers terminés (mode automatique,
/// la salle reste alimentée par la bibliothèque), soit une sélection explicite.
fn shared_ids() -> Vec<String> {
	if SHARE_ALL.load(Ordering::SeqCst) {
		completed_download_ids()
	} else {
		SHARED.lock().unwrap().clone()
	}
}

/// `(identifiants, mode automatique)` : sans liste fournie, on partage tout.
fn selected_from(payload: &Value) -> (Vec<String>, bool) {
	match payload.get("files").and_then(|value| value.as_array()) {
		Some(items) => (
			items
				.iter()
				.filter_map(|item| item.as_str().map(String::from))
				.collect(),
			false,
		),
		None => (completed_download_ids(), true),
	}
}

/// Démarre (ou redémarre) la salle de partage.
pub async fn start(payload: &Value) -> Result<Value> {
	let (selected, all) = selected_from(payload);
	stop_active();
	SHARE_ALL.store(all, Ordering::SeqCst);
	*SHARED.lock().unwrap() = selected.clone();

	let want_port = config().transfer_port;
	let listener = match TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], want_port))).await {
		Ok(listener) => listener,
		Err(_) => TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
			.await
			.map_err(|error| {
				ApiError::new(500, format!("Impossible d'ouvrir le port de partage : {error}"))
			})?,
	};
	let port = listener
		.local_addr()
		.map(|addr| addr.port())
		.unwrap_or(want_port);
	let ip = primary_ip();
	let token = make_token();

	let router = public_router(&token);
	let (tx, rx) = oneshot::channel::<()>();
	tokio::spawn(async move {
		let _ = axum::serve(listener, router.into_make_service())
			.with_graceful_shutdown(async move {
				let _ = rx.await;
			})
			.await;
	});

	let active = Active {
		token,
		port,
		ip,
		shutdown: Some(tx),
	};
	let url = active.url();
	*ACTIVE.lock().unwrap() = Some(active);

	Ok(json!({
		"running": true,
		"url": url,
		"port": port,
		"shared": selected,
		"sharedAll": all,
		"received": RECEIVED.lock().unwrap().iter().map(received_room_file).collect::<Vec<_>>(),
	}))
}

fn stop_active() {
	if let Some(mut active) = ACTIVE.lock().unwrap().take() {
		if let Some(tx) = active.shutdown.take() {
			let _ = tx.send(());
		}
	}
}

/// Arrête la salle de partage.
pub fn stop() -> Value {
	stop_active();
	status()
}

/// Change la liste des téléchargements exposés (sans redémarrer le serveur).
///
/// `{ "all": true }` réactive le partage automatique de toute la bibliothèque.
pub fn set_shared(payload: &Value) -> Result<Value> {
	if payload.get("all").and_then(|value| value.as_bool()) == Some(true) {
		SHARE_ALL.store(true, Ordering::SeqCst);
		*SHARED.lock().unwrap() = completed_download_ids();
		return Ok(status());
	}

	let files: Vec<String> = payload
		.get("files")
		.and_then(|value| value.as_array())
		.ok_or_else(|| ApiError::bad_request("Liste de fichiers requise"))?
		.iter()
		.filter_map(|item| item.as_str().map(String::from))
		.collect();
	SHARE_ALL.store(false, Ordering::SeqCst);
	*SHARED.lock().unwrap() = files;
	Ok(status())
}

/// Supprime un fichier reçu (disque + manifeste).
pub fn remove_received(id: &str) -> Result<Value> {
	let removed = {
		let mut files = RECEIVED.lock().unwrap();
		let index = files
			.iter()
			.position(|file| file.id == id)
			.ok_or_else(|| ApiError::not_found("Fichier reçu introuvable"))?;
		files.remove(index)
	};
	let _ = std::fs::remove_file(transfer_dir().join(&removed.stored_name));
	let snapshot = RECEIVED.lock().unwrap().clone();
	persist_received(&snapshot);
	Ok(json!({ "id": id, "removed": true }))
}

/// Vide tous les fichiers reçus.
pub fn clear_received() -> Value {
	let files: Vec<RoomFile> = RECEIVED.lock().unwrap().drain(..).collect();
	for file in &files {
		let _ = std::fs::remove_file(transfer_dir().join(&file.stored_name));
	}
	persist_received(&[]);
	json!({ "removed": files.len() })
}

/// Sert un fichier reçu depuis l'API locale (lecture ou enregistrement).
pub async fn received_file(id: &str, range: Option<String>, download: bool) -> Result<Response> {
	let (path, name) = {
		let files = RECEIVED.lock().unwrap();
		let file = files
			.iter()
			.find(|file| file.id == id)
			.ok_or_else(|| ApiError::not_found("Fichier reçu introuvable"))?;
		(transfer_dir().join(&file.stored_name), file.name.clone())
	};

	serve_path(path, &name, range, download).await
}

/// Normalise l'adresse d'une salle (`http://ip:port/jeton/`).
fn normalize_room_url(raw: &str) -> Result<String> {
	let text = raw.trim();
	if text.is_empty() {
		return Err(ApiError::bad_request("Adresse de salle vide"));
	}

	// Le QR code peut contenir l'URL complète ou seulement `ip:port/jeton`.
	let candidate = if text.starts_with("http://") || text.starts_with("https://") {
		text.to_string()
	} else {
		format!("http://{text}")
	};

	let mut url = reqwest::Url::parse(&candidate)
		.map_err(|_| ApiError::bad_request("Adresse de salle invalide"))?;
	if url.scheme() != "http" && url.scheme() != "https" {
		return Err(ApiError::bad_request("Seules les adresses http(s) sont acceptées"));
	}
	if url.path().trim_matches('/').is_empty() {
		return Err(ApiError::bad_request("Ce QR code ne contient pas de salle"));
	}

	url.set_query(None);
	url.set_fragment(None);
	if !url.path().ends_with('/') {
		let path = format!("{}/", url.path());
		url.set_path(&path);
	}
	Ok(url.to_string())
}

/// Récupère la liste des fichiers d'une salle distante (hors envois de cet appareil).
async fn fetch_remote_files(base: &str, client_id: &str) -> Result<Vec<Value>> {
	let response = crate::http::client()
		.get(format!("{base}files"))
		.query(&[("client", client_id)])
		.timeout(Duration::from_secs(8))
		.send()
		.await
		.map_err(|_| {
			ApiError::upstream(502, "Salle injoignable : vérifie le Wi-Fi et l'adresse")
		})?;

	if !response.status().is_success() {
		return Err(ApiError::upstream(
			502,
			format!("La salle a répondu {}", response.status().as_u16()),
		));
	}

	let payload: Value = response
		.json()
		.await
		.map_err(|_| ApiError::upstream(502, "Réponse de la salle illisible"))?;
	Ok(payload
		.get("files")
		.and_then(|value| value.as_array())
		.cloned()
		.unwrap_or_default())
}

fn remote_room() -> Result<RemoteRoom> {
	REMOTE
		.lock()
		.unwrap()
		.clone()
		.ok_or_else(|| ApiError::bad_request("Aucune salle rejointe"))
}

/// Rejoint une salle existante (QR code ou adresse saisie).
pub async fn join(payload: &Value) -> Result<Value> {
	let raw = payload
		.get("url")
		.and_then(|value| value.as_str())
		.ok_or_else(|| ApiError::bad_request("Adresse de salle requise"))?;
	let base = normalize_room_url(raw)?;
	let client_id = app_client_id();
	let files = fetch_remote_files(&base, &client_id).await?;

	*REMOTE.lock().unwrap() = Some(RemoteRoom {
		base: base.clone(),
		client_id,
	});

	Ok(json!({ "url": base, "files": files }))
}

/// Quitte la salle rejointe.
pub fn leave() -> Value {
	*REMOTE.lock().unwrap() = None;
	json!({ "left": true })
}

/// Liste à jour des fichiers de la salle rejointe.
pub async fn remote_files() -> Result<Value> {
	let Some(room) = REMOTE.lock().unwrap().clone() else {
		return Ok(json!({ "files": [] }));
	};
	let files = fetch_remote_files(&room.base, &room.client_id).await?;
	Ok(json!({ "files": files }))
}

/// Télécharge un fichier de la salle rejointe en relayant les octets (Range inclus).
pub async fn remote_download(id: &str, range: Option<String>) -> Result<Response> {
	let room = remote_room()?;
	let url = format!("{}file/{}", room.base, crate::services::providers::urlencode(id));

	let mut request = crate::http::client().get(url);
	if let Some(value) = range {
		request = request.header("range", value);
	}
	let response = request
		.send()
		.await
		.map_err(|_| ApiError::upstream(502, "Téléchargement depuis la salle impossible"))?;

	let status = response.status();
	if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
		return Err(ApiError::upstream(
			502,
			format!("La salle a refusé le fichier ({})", status.as_u16()),
		));
	}

	let mut builder = Response::builder().status(status.as_u16());
	for name in [
		header::CONTENT_TYPE,
		header::CONTENT_LENGTH,
		header::CONTENT_RANGE,
		header::CONTENT_DISPOSITION,
		header::ACCEPT_RANGES,
	] {
		if let Some(value) = response.headers().get(&name) {
			builder = builder.header(name, value);
		}
	}

	use futures::TryStreamExt;
	let stream = response.bytes_stream().map_err(std::io::Error::other);
	builder
		.body(Body::from_stream(stream))
		.map_err(|_| ApiError::new(500, "Réponse de téléchargement invalide"))
}

/// Envoie des téléchargements terminés de l'appareil vers la salle rejointe.
pub async fn remote_share(payload: &Value) -> Result<Value> {
	let room = remote_room()?;
	let ids: Vec<String> = payload
		.get("files")
		.and_then(|value| value.as_array())
		.ok_or_else(|| ApiError::bad_request("Liste de fichiers requise"))?
		.iter()
		.filter_map(|item| item.as_str().map(String::from))
		.collect();

	let mut sent: Vec<String> = Vec::new();
	for id in ids {
		let entry = manager().entry(&id)?;
		if entry.status != "completed" {
			continue;
		}

		let file = tokio::fs::File::open(file_path_of(&entry))
			.await
			.map_err(|error| ApiError::new(500, format!("Lecture impossible : {error}")))?;
		let part = reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(
			tokio_util::io::ReaderStream::new(file),
		))
		.file_name(entry.file_name.clone());
		let form = reqwest::multipart::Form::new()
			.text("uploader", "Movie Box")
			.text("client", room.client_id.clone())
			.part("file", part);

		let response = crate::http::client()
			.post(format!("{}upload", room.base))
			.multipart(form)
			.send()
			.await
			.map_err(|_| ApiError::upstream(502, "Envoi vers la salle impossible"))?;
		if !response.status().is_success() {
			return Err(ApiError::upstream(
				502,
				format!("La salle a refusé l'envoi ({})", response.status().as_u16()),
			));
		}
		sent.push(entry.file_name);
	}

	let files = fetch_remote_files(&room.base, &room.client_id).await?;
	Ok(json!({ "sent": sent, "files": files }))
}

/// Routeur public exposé sur le réseau local (protégé par le jeton dans le chemin).
///
/// Le jeton est inscrit en dur dans les routes : une URL au jeton erroné ne
/// correspond à aucune route et reçoit un 404.
pub fn public_router(token: &str) -> Router {
	let base = format!("/{token}");
	let limit = config().downloads.max_bytes.min(usize::MAX as u64) as usize;
	Router::new()
		.route(&base, get(page))
		.route(&format!("{base}/"), get(page))
		.route(&format!("{base}/files"), get(files))
		.route(&format!("{base}/file/{{id}}"), get(download))
		.route(&format!("{base}/upload"), post(upload))
		.layer(DefaultBodyLimit::max(limit))
}

async fn page() -> Html<&'static str> {
	Html(PAGE_HTML)
}

#[derive(Deserialize, Default)]
struct FilesQuery {
	#[serde(default)]
	client: Option<String>,
}

async fn files(Query(query): Query<FilesQuery>) -> Json<Value> {
	Json(json!({ "files": visible_files(query.client.as_deref()) }))
}

async fn download(Path((_token, id)): Path<(String, String)>, request: Request) -> Result<Response> {
	let range = request
		.headers()
		.get(header::RANGE)
		.and_then(|value| value.to_str().ok())
		.map(|value| value.trim().to_string());
	let (path, name) = resolved_path(&id)?;
	serve_path(path, &name, range, true).await
}

async fn upload(mut multipart: Multipart) -> Result<Json<Value>> {
	let dir = transfer_dir();
	std::fs::create_dir_all(&dir)
		.map_err(|error| ApiError::new(500, format!("Dossier de partage indisponible : {error}")))?;

	let mut added: Vec<Value> = Vec::new();
	let mut uploader = String::new();
	let mut uploader_id = String::new();

	while let Some(mut field) = multipart
		.next_field()
		.await
		.map_err(|error| ApiError::bad_request(format!("Envoi invalide : {error}")))?
	{
		let field_name = field.name().unwrap_or("").to_string();
		let Some(raw_name) = field.file_name().map(|value| value.to_string()) else {
			if field_name == "uploader" {
				if let Ok(text) = field.text().await {
					uploader = text.trim().chars().take(40).collect();
				}
			} else if field_name == "client" {
				if let Ok(text) = field.text().await {
					uploader_id = text.trim().chars().take(64).collect();
				}
			}
			continue;
		};

		let name = safe_file_name(&raw_name);
		let id = make_file_id();
		let stored_name = format!("{id}{}", extension_of(&name));
		let dest = dir.join(&stored_name);

		let mut file = tokio::fs::File::create(&dest)
			.await
			.map_err(|error| ApiError::new(500, format!("Écriture impossible : {error}")))?;
		let mut size: u64 = 0;
		while let Some(chunk) = field
			.chunk()
			.await
			.map_err(|error| ApiError::bad_request(format!("Envoi interrompu : {error}")))?
		{
			size += chunk.len() as u64;
			file.write_all(&chunk)
				.await
				.map_err(|error| ApiError::new(500, format!("Écriture impossible : {error}")))?;
		}
		let _ = file.flush().await;

		let entry = RoomFile {
			id,
			name: name.clone(),
			size,
			mime: mime_for_name(&name),
			uploader: uploader.clone(),
			uploader_id: uploader_id.clone(),
			added_at: now_ms(),
			stored_name,
		};
		RECEIVED.lock().unwrap().push(entry.clone());
		added.push(received_room_file(&entry));
	}

	let snapshot = RECEIVED.lock().unwrap().clone();
	persist_received(&snapshot);
	Ok(Json(json!({ "files": added })))
}

/// Page web servie aux appareils du réseau (auto-suffisante, thème sombre).
const PAGE_HTML: &str = r#"<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Movie Box — Partage Wi-Fi</title>
<style>
:root{color-scheme:dark}
*{box-sizing:border-box}
body{margin:0;min-height:100vh;font-family:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;background:#0b0d12;color:#eef1f6}
.wrap{max-width:640px;margin:0 auto;padding:22px 18px 60px}
header{display:flex;align-items:center;gap:12px;margin-bottom:20px}
.logo{width:46px;height:46px;border-radius:15px;background:rgba(124,92,255,.16);color:#b9a6ff;display:grid;place-items:center;font-size:22px;flex:none}
h1{font-size:20px;margin:0}
.sub{color:#8b93a7;font-size:13px;margin:3px 0 0}
.card{background:#151925;border:1px solid #232a3a;border-radius:16px;padding:16px;margin-bottom:16px}
.card h2{font-size:14px;text-transform:uppercase;letter-spacing:.06em;color:#8b93a7;margin:0 0 12px}
label{font-size:13px;color:#aab2c5;display:block;margin-bottom:6px}
input[type=text]{width:100%;padding:11px 12px;border-radius:11px;border:1px solid #2c3446;background:#0f1320;color:#eef1f6;font-size:15px}
input[type=text]:focus{outline:2px solid rgba(124,92,255,.5);border-color:transparent}
.drop{display:block;margin-top:12px;padding:22px 14px;border:1.5px dashed #3a4358;border-radius:12px;text-align:center;color:#9aa3b8;cursor:pointer;transition:border-color .15s,background .15s}
.drop:hover,.drop.over{border-color:#7c5cff;background:rgba(124,92,255,.06)}
.drop strong{color:#cdd3e2}
.drop input{display:none}
.barwrap{height:8px;background:#0f1320;border-radius:999px;overflow:hidden;margin-top:12px;display:none}
.barwrap.on{display:block}
.bar{height:100%;width:0;background:linear-gradient(90deg,#7c5cff,#b9a6ff);transition:width .15s}
.status{font-size:13px;color:#9aa3b8;margin-top:10px;min-height:18px}
ul.files{list-style:none;margin:0;padding:0}
li.file{display:flex;align-items:center;gap:12px;padding:11px 0;border-top:1px solid #1e2434}
li.file:first-child{border-top:none}
.ficon{width:38px;height:38px;border-radius:11px;background:#0f1320;color:#8b93a7;display:grid;place-items:center;flex:none;font-size:17px}
.fmeta{min-width:0;flex:1}
.fname{font-size:14px;font-weight:600;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.fsub{font-size:12px;color:#8b93a7;margin-top:2px}
a.dl,button.go{background:#7c5cff;color:#fff;border:none;border-radius:10px;padding:9px 14px;font-size:13px;font-weight:600;text-decoration:none;cursor:pointer;white-space:nowrap}
a.dl:hover,button.go:hover{background:#8d70ff}
.empty{color:#8b93a7;font-size:14px;text-align:center;padding:14px 0}
.tag{display:inline-block;font-size:11px;color:#b9a6ff;background:rgba(124,92,255,.14);border-radius:6px;padding:1px 6px;margin-left:6px}
.hint{font-size:12px;color:#6f7889;margin-top:10px}
</style>
</head>
<body>
<div class="wrap">
  <header>
    <div class="logo">&#9776;</div>
    <div>
      <h1>Movie Box — Partage Wi-Fi</h1>
      <p class="sub">Envoyez et recevez des fichiers avec les appareils de ce reseau.</p>
    </div>
  </header>

  <div class="card">
    <h2>Envoyer un fichier</h2>
    <label for="who">Votre nom (optionnel)</label>
    <input type="text" id="who" placeholder="Ex. Awa" autocomplete="off">
    <label class="drop" id="drop">
      <strong>Choisir des fichiers</strong><br>ou les deposer ici
      <input type="file" id="pick" multiple>
    </label>
    <div class="barwrap" id="barwrap"><div class="bar" id="bar"></div></div>
    <div class="status" id="status"></div>
  </div>

  <div class="card">
    <h2>Fichiers disponibles <span id="count" class="tag">0</span></h2>
    <ul class="files" id="list"><li class="empty">Chargement...</li></ul>
    <p class="hint">Les fichiers envoyes ici restent sur l'appareil qui heberge le partage.</p>
  </div>
</div>

<script>
(function(){
  var base = location.pathname;
  if (base.charAt(base.length-1) !== '/') base += '/';
  var list = document.getElementById('list');
  var count = document.getElementById('count');
  var status = document.getElementById('status');
  var drop = document.getElementById('drop');
  var pick = document.getElementById('pick');
  var bar = document.getElementById('bar');
  var barwrap = document.getElementById('barwrap');
  var who = document.getElementById('who');

  who.value = localStorage.getItem('moviebox-name') || '';
  who.addEventListener('change', function(){ localStorage.setItem('moviebox-name', who.value.trim()); });

  // Identifiant de cet appareil : la salle masque les fichiers qu'il a envoyes.
  var clientId = localStorage.getItem('moviebox-client');
  if(!clientId){
    clientId = 'web-' + Math.random().toString(36).slice(2, 10) + Date.now().toString(36);
    localStorage.setItem('moviebox-client', clientId);
  }

  function esc(s){ return String(s==null?'':s).replace(/[&<>"']/g, function(c){ return {'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]; }); }
  function fmt(b){ b=Number(b)||0; if(b<=0) return '0 o'; var u=['o','Ko','Mo','Go','To']; var i=Math.min(u.length-1,Math.floor(Math.log(b)/Math.log(1024))); return (b/Math.pow(1024,i)).toFixed(i?1:0).replace('.',',')+' '+u[i]; }
  function icon(m){ if(/^video\//.test(m)) return '&#9654;'; if(/^image\//.test(m)) return '&#9635;'; if(/^audio\//.test(m)) return '&#9835;'; if(/zip|rar|7z|tar/.test(m)) return '&#128230;'; return '&#128196;'; }

  function render(files){
    count.textContent = files.length;
    if(!files.length){ list.innerHTML = '<li class="empty">Aucun fichier pour le moment.</li>'; return; }
    list.innerHTML = files.map(function(f){
      var from = f.uploader ? '<span class="tag">'+esc(f.uploader)+'</span>' : '';
      return '<li class="file"><span class="ficon">'+icon(f.mime||'')+'</span>'
        + '<div class="fmeta"><div class="fname">'+esc(f.name)+from+'</div>'
        + '<div class="fsub">'+fmt(f.size)+'</div></div>'
        + '<a class="dl" href="'+base+'file/'+encodeURIComponent(f.id)+'" download>T&eacute;l&eacute;charger</a></li>';
    }).join('');
  }

  function load(){
    fetch(base+'files?client='+encodeURIComponent(clientId), {cache:'no-store'})
      .then(function(r){ return r.json(); })
      .then(function(d){ render(d.files||[]); })
      .catch(function(){ list.innerHTML = '<li class="empty">Connexion au partage perdue.</li>'; });
  }

  function send(files){
    if(!files || !files.length) return;
    var fd = new FormData();
    var name = who.value.trim();
    if(name) fd.append('uploader', name);
    fd.append('client', clientId);
    for(var i=0;i<files.length;i++) fd.append('file', files[i], files[i].name);
    var xhr = new XMLHttpRequest();
    xhr.open('POST', base+'upload');
    barwrap.classList.add('on');
    xhr.upload.onprogress = function(e){ if(e.lengthComputable) bar.style.width = (e.loaded/e.total*100)+'%'; };
    xhr.onload = function(){
      bar.style.width = '0'; barwrap.classList.remove('on');
      status.textContent = xhr.status < 300 ? 'Envoi termine.' : 'Echec de l\'envoi.';
      load();
    };
    xhr.onerror = function(){ barwrap.classList.remove('on'); status.textContent = 'Connexion perdue.'; };
    xhr.send(fd);
  }

  pick.addEventListener('change', function(){ send(pick.files); pick.value=''; });
  ['dragenter','dragover'].forEach(function(t){ drop.addEventListener(t, function(e){ e.preventDefault(); drop.classList.add('over'); }); });
  ['dragleave','drop'].forEach(function(t){ drop.addEventListener(t, function(e){ e.preventDefault(); drop.classList.remove('over'); }); });
  drop.addEventListener('drop', function(e){ if(e.dataTransfer) send(e.dataTransfer.files); });

  load();
  setInterval(load, 4000);
})();
</script>
</body>
</html>
"#;
