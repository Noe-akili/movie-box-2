import axios from "axios";
import { usePending } from "../stores/loading";
import { useSettings } from "../stores/settings";

/**
 * Origine du serveur embarque, definie au demarrage par l'app Tauri Android
 * (`window.__SERVER_ORIGIN__`, ex. « http://127.0.0.1:5174 »). Vide en web.
 */
export const serverOrigin = () =>
	typeof window !== "undefined" && window.__SERVER_ORIGIN__ ? window.__SERVER_ORIGIN__ : "";

/** Base de l'API : origine runtime (Android) sinon chemin relatif (web). */
export const apiBase = () =>
	(typeof window !== "undefined" && window.__API_BASE_URL__) ||
	import.meta.env.VITE_API_BASE_URL ||
	`${serverOrigin()}/api`;

/**
 * Absolutise une URL servie par notre backend (`/api/...`) pour l'app Android :
 * sans cela, un `<video src="/api/...">` viserait l'origine `tauri://localhost`.
 */
export const mediaUrl = (url) =>
	url && url.startsWith("/") && serverOrigin() ? `${serverOrigin()}${url}` : url;

/**
 * Client HTTP unique de l'application.
 * - toutes les requetes passent par le backend Rust (/api) : la cle TMDB reste cote serveur
 * - la langue et la region choisies par l'utilisateur sont ajoutees automatiquement
 * - les intercepteurs renvoient directement le JSON (plus de .data a ecrire partout)
 */
const http = axios.create({
	baseURL: apiBase(),
	// Largeur sur le budget reponse du serveur (12 s sur /home) : laisse le temps
	// aux appels plus longs (fiche, saison) sans tomber en timeout premature.
	timeout: 45_000,
});

http.interceptors.request.use((request) => {
	const settings = useSettings();

	// L'origine du serveur embarque n'est connue qu'apres le bootstrap Tauri.
	request.baseURL = apiBase();

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
					: "Impossible de joindre le serveur Movie Box (lance « npm run dev:all »)");

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

export default http;
