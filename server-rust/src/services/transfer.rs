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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Multipart, Path, Request};
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
pub fn visible_files() -> Vec<Value> {
	let mut out = Vec::new();
	let ids: Vec<String> = SHARED.lock().unwrap().clone();
	for id in ids {
		if let Some(item) = shared_room_file(&id) {
			out.push(item);
		}
	}
	for file in RECEIVED.lock().unwrap().iter() {
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
	let shared: Vec<String> = SHARED.lock().unwrap().clone();

	match ACTIVE.lock().unwrap().as_ref() {
		Some(active) => json!({
			"running": true,
			"token": active.token,
			"port": active.port,
			"url": active.url(),
			"shared": shared,
			"received": received,
			"files": visible_files(),
		}),
		None => json!({
			"running": false,
			"port": Value::Null,
			"url": Value::Null,
			"shared": shared,
			"received": received,
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

fn selected_from(payload: &Value) -> Vec<String> {
	payload
		.get("files")
		.and_then(|value| value.as_array())
		.map(|items| {
			items
				.iter()
				.filter_map(|item| item.as_str().map(String::from))
				.collect()
		})
		.unwrap_or_else(completed_download_ids)
}

/// Démarre (ou redémarre) la salle de partage.
pub async fn start(payload: &Value) -> Result<Value> {
	let selected = selected_from(payload);
	stop_active();
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
pub fn set_shared(payload: &Value) -> Result<Value> {
	let files: Vec<String> = payload
		.get("files")
		.and_then(|value| value.as_array())
		.ok_or_else(|| ApiError::bad_request("Liste de fichiers requise"))?
		.iter()
		.filter_map(|item| item.as_str().map(String::from))
		.collect();
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

async fn files() -> Json<Value> {
	Json(json!({ "files": visible_files() }))
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
    fetch(base+'files', {cache:'no-store'})
      .then(function(r){ return r.json(); })
      .then(function(d){ render(d.files||[]); })
      .catch(function(){ list.innerHTML = '<li class="empty">Connexion au partage perdue.</li>'; });
  }

  function send(files){
    if(!files || !files.length) return;
    var fd = new FormData();
    var name = who.value.trim();
    if(name) fd.append('uploader', name);
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
