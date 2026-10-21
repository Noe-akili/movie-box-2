import axios from "axios";
import { usePending } from "../stores/loading";
import { useSettings } from "../stores/settings";

/**
 * Origine virtuelle du backend embarqué, définie au démarrage par l'app Tauri
 * (`window.__SERVER_ORIGIN__`). Vide en web.
 *
 * L'API passe toujours par l'IPC Tauri. Les flux média utilisent, sur Android,
 * un serveur HTTP loopback (`http://127.0.0.1:<port>`) — le protocole
 * `moviebox://` ne sait pas honorer les requêtes `Range` (seek) — et le
 * protocole `moviebox://` ailleurs.
 */
/**
 * Origine de repli du protocole média embarqué. Utilisée si la commande
 * `server_origin` n'a pas répondu : sans origine, les balises `<img>`/`<video>`
 * viseraient `tauri://localhost` et aucun média ne s'afficherait.
 */
const FALLBACK_SERVER_ORIGIN =
	typeof navigator !== "undefined" && /android/i.test(navigator.userAgent)
		? "http://moviebox.localhost"
		: "moviebox://localhost";

/** Vrai lorsqu'on tourne dans la WebView Tauri (Android/desktop). */
const isTauri = () =>
	typeof window !== "undefined" &&
	Boolean(window.__TAURI__ || window.__TAURI_INTERNALS__);

export const serverOrigin = () => {
	if (typeof window === "undefined") return "";
	if (window.__SERVER_ORIGIN__) return window.__SERVER_ORIGIN__;
	// Repli : en app packagée, le protocole média est exposé sous une origine
	// connue même si la commande `server_origin` a échoué.
	return isTauri() ? FALLBACK_SERVER_ORIGIN : "";
};

/** Base de l'API : origine runtime (Android) sinon chemin relatif (web). */
export const apiBase = () =>
	(typeof window !== "undefined" && window.__API_BASE_URL__) ||
	import.meta.env.VITE_API_BASE_URL ||
	`${serverOrigin()}/api`;

/**
 * Absolutise une URL média servie par notre backend (`/api/...`).
 * Sur Android, elle devient `http://127.0.0.1:<port>/api/...` (serveur média
 * loopback) ; sur bureau, `moviebox://localhost/api/...` (protocole Tauri).
 */
export const mediaUrl = (url) =>
	url && url.startsWith("/") && serverOrigin() ? `${serverOrigin()}${url}` : url;

/**
 * Passe un flux distant par le proxy local du backend (contourne les blocages
 * anti-hotlink / CORS de la WebView et garde le seek via les requêtes Range).
 */
export const proxyStreamUrl = (url) =>
	`/api/internal/proxy?src=${encodeURIComponent(url)}`;

/** `invoke` de Tauri, résolu au moment de l'appel. */
function tauriInvoke() {
	if (typeof window === "undefined") return null;
	return (
		window.__TAURI__?.core?.invoke ||
		window.__TAURI__?.invoke ||
		window.__TAURI_INTERNALS__?.invoke ||
		null
	);
}

/** Chemin de la requête, sans domaine (ex. « /api/home »). */
function requestPath(config) {
	const full = `${config.baseURL || ""}${config.url || ""}`;
	try {
		return new URL(full, "http://localhost").pathname;
	} catch {
		return config.url || "/";
	}
}

/** Sérialise les paramètres de requête (lang, region, …). */
function requestQuery(params) {
	if (!params) return "";
	const search = new URLSearchParams();
	for (const [key, value] of Object.entries(params)) {
		if (value === undefined || value === null) continue;
		if (Array.isArray(value)) value.forEach((item) => search.append(key, item));
		else search.append(key, value);
	}
	return search.toString();
}

/**
 * Adaptateur axios : route toutes les requêtes vers la commande Tauri
 * `api_request`, qui exécute le routeur Rust en mémoire (aucun réseau).
 */
function tauriAdapter(config) {
	return new Promise((resolve, reject) => {
		const method = (config.method || "get").toUpperCase();
		const path = requestPath(config);
		const query = requestQuery(config.params);
		const body =
			typeof config.data === "string"
				? config.data
				: config.data !== undefined && config.data !== null
					? JSON.stringify(config.data)
					: null;

		const invoke = tauriInvoke();
		if (!invoke) {
			reject(new Error("Tauri IPC indisponible"));
			return;
		}

		invoke("api_request", { method, path, query, body })
			.then((result) => {
				let data = result.body ?? null;
				if (typeof result.body === "string" && result.body.length > 0) {
					try {
						data = JSON.parse(result.body);
					} catch {
						data = result.body;
					}
				}

				const response = {
					data,
					status: result.status,
					statusText: "",
					headers: {},
					config,
					request: null,
				};

				const validate =
					typeof config.validateStatus === "function"
						? config.validateStatus
						: (status) => status >= 200 && status < 300;

				if (validate(result.status)) {
					resolve(response);
				} else {
					reject(
						Object.assign(new Error(`Request failed with status code ${result.status}`), {
							isAxiosError: true,
							code: `ERR_${result.status}`,
							config,
							response,
							status: result.status,
						}),
					);
				}
			})
			.catch((error) => reject(error));
	});
}

/**
 * Client HTTP unique de l'application.
 * - toutes les requêtes passent par le backend Rust (IPC Tauri ou /api sur le web) :
 *   la clé TMDB reste côté serveur
 * - la langue et la région choisies par l'utilisateur sont ajoutées automatiquement
 * - les intercepteurs renvoient directement le JSON (plus de .data à écrire partout)
 */
const http = axios.create({
	baseURL: apiBase(),
	// Largeur sur le budget réponse du serveur (12 s sur /home) : laisse le temps
	// aux appels plus longs (fiche, saison) sans tomber en timeout premature.
	timeout: 45_000,
});

// L'adaptateur Tauri est sélectionné dynamiquement dans l'intercepteur de
// requête : il dépend de la présence de `window.__TAURI__` au moment de l'appel
// (injection parfois différée sur Android).

http.interceptors.request.use((request) => {
	const settings = useSettings();

	// L'origine du serveur embarque n'est connue qu'apres le bootstrap Tauri.
	request.baseURL = apiBase();

	if (tauriInvoke()) {
		request.adapter = tauriAdapter;
	}

	request.params = {
		lang: settings.value.language,
		region: settings.value.region,
		...(request.params || {}),
	};

	// Barre de progression : chaque requête lancée donne un retour visuel immédiat
	// (l'utilisateur ne croit pas que l'app a plante pendant un appel lent).
	usePending().beginRequest();

	return request;
});

// Fin de requête : le compteur de la barre est décrémenté dans les deux cas.
const endRequest = () => usePending().endRequest();

http.interceptors.response.use(
	(response) => {
		endRequest();
		return response.data;
	},
	(error) => {
		endRequest();
		const status = error.response?.status;
		const isTimeout = error.code === "ECONNABORTED" || /timeout/i.test(error.message || "");
		const message =
			error.response?.data?.error?.message ||
			(isTimeout
				? "Le serveur met trop de temps à répondre. Réessaie dans quelques secondes."
				: status
					? `Erreur ${status}`
					: "Impossible de joindre le serveur Movie Box.");

		return Promise.reject(Object.assign(new Error(message), { status }));
	},
);

export const fetchHome = (params) => http.get("/home", { params });
export const fetchGenres = (type = "movie") => http.get("/genres", { params: { type } });
export const searchTitles = (params) => http.get("/search", { params });
export const fetchDiscover = (mediaType, params) => http.get(`/discover/${mediaType}`, { params });
export const fetchFreeTitles = (params) => http.get("/free", { params });
export const fetchTitle = (mediaType, id) =>
	http.get(`/title/${mediaType}/${encodeURIComponent(id)}`);
export const fetchSeason = (tvId, seasonNumber) => http.get(`/title/tv/${tvId}/season/${seasonNumber}`);
export const fetchInternalStreams = (id, params) => http.get(`/internal/stream/${encodeURIComponent(id)}`, { params });
export const fetchLegalInfo = () => http.get("/legal");
export const fetchHealth = () => http.get("/health");

/** Vrai dans la WebView Tauri (l'IPC est alors disponible). */
export const isTauriRuntime = isTauri;

/**
 * Récupère le journal de diagnostic du backend Rust (commande `backend_diag`).
 * Accessible uniquement dans l'application (hors navigateur web).
 */
export const fetchBackendDiag = () => {
	const invoke = tauriInvoke();
	if (!invoke) {
		return Promise.reject(new Error("Diagnostic disponible uniquement dans l'application."));
	}
	return invoke("backend_diag");
};

// --- Telechargements : le backend stocke les films et episodes libres de droits ---
export const fetchDownloads = () => http.get("/downloads");
export const fetchDownload = (id) => http.get(`/downloads/${encodeURIComponent(id)}`);
export const createDownload = (payload) => http.post("/downloads", payload);
export const retryDownload = (id) => http.post(`/downloads/${encodeURIComponent(id)}/retry`);
export const cancelDownload = (id) => http.post(`/downloads/${encodeURIComponent(id)}/cancel`);
export const deleteDownload = (id) => http.delete(`/downloads/${encodeURIComponent(id)}`);

/** URLs directes servies par le backend (lecture du fichier local / enregistrement). */
export const downloadStreamUrl = (id) =>
	mediaUrl(`/api/downloads/${encodeURIComponent(id)}/stream`);
export const downloadFileUrl = (id) =>
	mediaUrl(`/api/downloads/${encodeURIComponent(id)}/file`);

// --- Partage Wi-Fi local : salle de transfert entre appareils du meme reseau ---
export const fetchTransfer = () => http.get("/transfer");
export const startTransfer = (payload = {}) => http.post("/transfer/start", payload);
export const stopTransfer = () => http.post("/transfer/stop");
export const updateTransferFiles = (files) => http.put("/transfer/files", { files });
export const shareAllTransferFiles = () => http.put("/transfer/files", { all: true });
export const removeTransferFile = (id) => http.delete(`/transfer/received/${encodeURIComponent(id)}`);
export const clearTransferFiles = () => http.delete("/transfer/received");
export const joinTransferRoom = (url) => http.post("/transfer/join", { url });
export const leaveTransferRoom = () => http.post("/transfer/leave");
export const fetchRemoteTransferFiles = () => http.get("/transfer/remote");
export const shareTransferDownloads = (files) => http.post("/transfer/remote/share", { files });
export const remoteTransferFileUrl = (id) =>
	mediaUrl(`/api/transfer/remote/file/${encodeURIComponent(id)}`);

export default http;
