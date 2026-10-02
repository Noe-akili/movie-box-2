import { createHash } from "node:crypto";
import { config } from "../config.js";
import { cached } from "../lib/cache.js";
import { ApiError, badRequest, notFound } from "../lib/errors.js";
import { buildUrl, fetchJson } from "../lib/http.js";

/**
 * Wikimedia Commons : deuxieme source de films gratuits et legaux.
 * Interet juridique : Commons n'accepte QUE des œuvres libres (domaine public ou licence Creative
 * Commons) — la licence de chaque fichier est verifiee par la communaute Wikimedia, et le libelle
 * de licence est affiche sur la fiche comme sur la page de lecture.
 *
 * Deux contraintes techniques propres a cette API :
 * - elle limite fortement le debit (HTTP 429) : tous les appels passent par une file serie + un cache long ;
 * - les URLs de fichiers se deduisent du chemin MD5 (`/wikipedia/commons/<a>/<ab>/<fichier>`),
 *   ce qui permet de reconstruire une URL de telechargement sans appel reseau supplementaire.
 */

const API_URL = `${config.commonsApiUrl}/w/api.php`;
const ID_PREFIX = "commons:";
const VARIANT_SEPARATOR = "::";
const VIDEO_MIME = /^video\/(webm|ogg|mp4|mpeg)$/i;
const PLAYABLE_TRANSCODE = /^video\/(webm|ogg|mp4|mpeg)/i;
const VIDEO_EXTENSION = /\.(webm|ogv|ogg|mp4|mpg|mpeg)$/i;

/** Categories verifiees cote API (contiennent uniquement des fichiers video). */
export const COMMONS_CATEGORIES = [
	{ id: "Films in the public domain", label: "Films du domaine public" },
	{ id: "Animated films", label: "Films d'animation" },
	{ id: "Silent films", label: "Films muets" },
	{ id: "Short films", label: "Courts-métrages" },
	{ id: "Documentary films", label: "Documentaires" },
];

export const DEFAULT_COMMONS_CATEGORY = COMMONS_CATEGORIES[0].id;

export const isCommonsId = (value) => String(value || "").startsWith(ID_PREFIX);
export const commonsFileId = (name) => `${ID_PREFIX}${name}`;
export const commonsKey = (value) => String(value || "").slice(ID_PREFIX.length);

/** Nom de fichier virtuel : `<fichier>` pour l'original, `<fichier>::<variante>` pour un transcodage. */
export const commonsFileName = (name, variant = "") => (variant ? `${name}${VARIANT_SEPARATOR}${variant}` : name);

export const splitVariant = (fileName) => {
	const [name, variant = ""] = String(fileName || "").split(VARIANT_SEPARATOR);
	return { name, variant };
};

const sanitize = (value) =>
	String(value || "")
		.replace(/["\\]/g, " ")
		.replace(/\s+/g, " ")
		.trim()
		.slice(0, 80);

const stripHtml = (value) =>
	String(value || "")
		.replace(/<[^>]*>/g, " ")
		.replace(/&amp;/g, "&")
		.replace(/&quot;/g, '"')
		.replace(/&#39;/g, "'")
		.replace(/&nbsp;/g, " ")
		.replace(/\s+/g, " ")
		.trim();

/** File d'attente : un seul appel a la fois, avec un espacement minimal (limite de debit Wikimedia). */
let queue = Promise.resolve();
let lastCallAt = 0;

const serialized = (task) => {
	const run = queue.then(async () => {
		const wait = Math.max(0, 300 - (Date.now() - lastCallAt));
		if (wait) await new Promise((resolve) => setTimeout(resolve, wait));

		try {
			return await task();
		} finally {
			lastCallAt = Date.now();
		}
	});

	queue = run.then(
		() => undefined,
		() => undefined,
	);

	return run;
};

const api = (params, { timeoutMs = 25_000, retries = 2 } = {}) =>
	serialized(() =>
		fetchJson(buildUrl(API_URL, { format: "json", origin: "*", ...params }), { timeoutMs, retries }),
	);

const INFO_PROPS = {
	prop: "videoinfo",
	viprop: "url|size|mime|mediatype|derivatives|extmetadata|canonicaltitle",
	viurlwidth: "480",
	viextmetadatafilter:
		"LicenseShortName|UsageTerms|LicenseUrl|Artist|ImageDescription|DateTimeOriginal|Credit|AttributionRequired",
};

/** Titre lisible a partir du nom de fichier Commons. */
const titleFromName = (name) =>
	String(name || "")
		.replace(/\.[a-z0-9]{2,5}$/i, "")
		.replace(/_+/g, " ")
		.replace(/\s*\((?:16mm|8mm|35mm|TV|HD|UHD|SD|4K)[^)]*\)/gi, " ")
		.replace(/\s*-\s*(?:16mm|8mm|35mm|silent|HD|UHD|flat|full)[^-]*$/i, " ")
		.replace(/\s{2,}/g, " ")
		.trim() || String(name || "");

const yearFrom = (value) => {
	const match = String(value || "").match(/(1[89]\d{2}|20\d{2})/);
	return match ? Number(match[1]) : null;
};

const licenseLabelOf = (extmetadata = {}) => {
	const short = stripHtml(extmetadata.LicenseShortName?.value);
	const usage = stripHtml(extmetadata.UsageTerms?.value).toLowerCase();

	if (usage.includes("public domain") || short.toLowerCase().includes("public domain")) return "Domaine public";
	if (/^cc0/i.test(short) || short.toLowerCase().includes("cc0")) return "CC0 — domaine public";
	if (short) return short;

	return "Licence libre (Wikimedia Commons)";
};

const qualityOf = (height, isOriginal) => {
	if (height >= 720) return "HD";
	if (height >= 480) return "SD";
	return isOriginal ? "Original" : "SD";
};

const containerOf = (mime) => {
	const value = String(mime || "").toLowerCase();
	if (value.includes("webm")) return "WebM";
	if (value.includes("ogg")) return "Ogg";
	if (value.includes("mp4") || value.includes("quicktime") || value.includes("mpeg4")) return "MP4";
	return "Vidéo";
};

/**
 * Nom de fichier tel qu'il apparait dans une URL Wikimedia : les espaces deviennent des
 * « _ » (`The Champion 1915….webm` -> `The_Champion_1915….webm`). Indispensable : une URL
 * construite avec `%20` repond 404, et l'empreinte MD5 du dossier est calculee sur cette
 * forme la (et non sur le titre avec espaces).
 */
const uploadName = (name) => String(name || "").replace(/ /g, "_");

/** Encode un chemin de fichier sans casser les separateurs ni les « _ ». */
const encodeUploadPath = (name) =>
	uploadName(name)
		.split("/")
		.map((segment) => encodeURIComponent(segment))
		.join("/");

/** Chemin MD5 utilise par Wikimedia : `/wikipedia/commons/<a>/<ab>/<fichier>`. */
const hashDir = (name) => {
	const digest = createHash("md5").update(uploadName(name)).digest("hex");
	return `${digest[0]}/${digest.slice(0, 2)}`;
};

export const commonsOriginalUrl = (name) =>
	`${config.commonsUploadUrl}/wikipedia/commons/${hashDir(name)}/${encodeUploadPath(name)}`;

const commonsTranscodeUrl = (name, variant) =>
	`${config.commonsUploadUrl}/wikipedia/commons/transcoded/${hashDir(name)}/${encodeUploadPath(
		name,
	)}/${encodeUploadPath(name)}.${variant}`;

/**
 * URLs confirmees par l'API (`videoinfo.url` et `derivatives[].src`) : elles sont utilisees en
 * priorite, la reconstruction n'etant qu'un repli (reprise d'un telechargement apres redemarrage).
 */
const knownUrls = new Map();
const rememberUrl = (key, url) => {
	if (url) knownUrls.set(key, url);
	return url;
};

/** Retire les parametres de suivi ajoutes par l'API (`?utm_source=…`). */
const cleanRemoteUrl = (value) => {
	const url = String(value || "").split("?")[0];
	return /^https:\/\//i.test(url) ? url : "";
};

/**
 * Taille reelle d'un fichier distant, mesuree par une requete `Range: bytes=0-0` (une poignee
 * d'octets). Necessaire car l'API ne donne la taille que de l'original : sans cette mesure, la
 * barre de progression d'un transcodage n'aurait aucun pourcentage. Resultat mis en cache.
 */
const probeRemoteSize = (url) =>
	cached(`commons:size:${url}`, config.ttl.long, async () => {
		try {
			const response = await fetch(url, {
				headers: { "user-agent": "movie-box/1.0", range: "bytes=0-0" },
				redirect: "follow",
				signal: AbortSignal.timeout(15_000),
			});

			await response.body?.cancel();
			if (!response.ok && response.status !== 206) return 0;

			const total = Number(String(response.headers.get("content-range") || "").split("/")[1]);
			return Number.isFinite(total) && total > 0 ? total : 0;
		} catch {
			return 0;
		}
	});

/**
 * Qualites disponibles : l'original + les transcodages WebM/MP4.
 * Les transcodages passent avant l'original a definition egale (bien plus legers a lire et a telecharger).
 */
const streamsFrom = (name, info) => {
	const originalHeight = Number(info.height || 0);
	const originalUrl = rememberUrl(
		commonsFileName(name),
		cleanRemoteUrl(info.url) || commonsOriginalUrl(name),
	);

	const streams = [
		{
			name: commonsFileName(name),
			format: `${containerOf(info.mime)}${originalHeight ? ` ${originalHeight}p` : ""}`,
			size: Number(info.size || 0),
			quality: qualityOf(originalHeight, true),
			url: originalUrl,
			score: originalHeight >= 720 ? 20 : 30,
		},
	];

	for (const derivative of info.derivatives || []) {
		const variant = String(derivative.transcodekey || "");
		if (!variant || !PLAYABLE_TRANSCODE.test(String(derivative.type || ""))) continue;

		const height = Number(derivative.height || 0);
		const url = rememberUrl(
			commonsFileName(name, variant),
			cleanRemoteUrl(derivative.src) || commonsTranscodeUrl(name, variant),
		);

		streams.push({
			name: commonsFileName(name, variant),
			format: `${containerOf(derivative.type)}${height ? ` ${height}p` : ""}`,
			size: Number(derivative.size || 0),
			quality: qualityOf(height, false),
			url,
			score: height >= 720 ? 5 : height >= 480 ? 10 : 15,
		});
	}

	return streams.sort((a, b) => a.score - b.score || b.size - a.size).map(({ score, ...stream }) => stream);
};

const fileNameOf = (page) => String(page?.title || "").replace(/^File:/, "").trim();

/** Carte normalisee, compatible avec les cartes Internet Archive utilisees par l'interface. */
export const commonsCard = (page) => {
	const info = (page?.videoinfo || [])[0];
	const name = fileNameOf(page);

	if (!info || !name || !VIDEO_MIME.test(String(info.mime || ""))) return null;

	const extmetadata = info.extmetadata || {};
	const description = stripHtml(extmetadata.ImageDescription?.value);
	const year = yearFrom(extmetadata.DateTimeOriginal?.value) || yearFrom(name);

	return {
		id: `commons-${createHash("sha1").update(name).digest("hex").slice(0, 12)}`,
		mediaType: "archive",
		archiveId: commonsFileId(name),
		sourceId: commonsFileId(name),
		source: "commons",
		sourceLabel: "Wikimedia Commons — domaine public / libre",
		badge: "GRATUIT",
		title: titleFromName(name),
		overview: description,
		poster: info.thumburl || null,
		thumb: info.thumburl || null,
		backdrop: info.thumburl || null,
		year,
		rating: null,
		downloads: null,
		creator: stripHtml(extmetadata.Artist?.value) || null,
		license: extmetadata.LicenseUrl?.value || null,
		licenseLabel: licenseLabelOf(extmetadata),
		collection: null,
		language: null,
		runtime: info.duration ? Math.round(Number(info.duration) / 60) : null,
		duration: Number(info.duration) || null,
		genres: [],
		genreIds: [],
	};
};

const buildQuery = ({ q, category }) => {
	const parts = ["filetype:video"];
	const term = sanitize(q);
	if (term) parts.push(term);
	if (category) parts.push(`incategory:"${sanitize(category)}"`);
	return parts.join(" ");
};

/**
 * Recherche paginee (1 seul appel reseau par page) : `generator=search` renvoie directement les
 * informations video de chaque fichier. `total` n'est pas fourni par Commons, `hasMore` s'appuie
 * donc sur le nombre d'elements recus.
 */
export const searchCommons = async ({ q = "", category = "", page = 1, rows = 24 } = {}) => {
	const safePage = Math.max(1, Number(page) || 1);
	const safeRows = Math.min(50, Math.max(1, Number(rows) || 24));
	const activeCategory = category || (sanitize(q) ? "" : DEFAULT_COMMONS_CATEGORY);

	return cached(`commons:search:${activeCategory}:${q}:${safePage}:${safeRows}`, config.ttl.medium, async () => {
		const payload = await api({
			action: "query",
			generator: "search",
			gsrsearch: buildQuery({ q, category: activeCategory }),
			gsrnamespace: "6",
			gsrlimit: String(safeRows),
			gsroffset: String((safePage - 1) * safeRows),
			...INFO_PROPS,
		});

		const pages = Object.values(payload?.query?.pages || {});
		const items = pages
			.map(commonsCard)
			.filter(Boolean)
			.sort((a, b) => (b.duration || 0) - (a.duration || 0));

		return {
			page: safePage,
			rows: safeRows,
			license: "free",
			total: null,
			hasMore: items.length >= safeRows,
			items,
		};
	});
};

const fetchInfo = async (fileNames) => {
	const unique = [...new Set(fileNames.filter(Boolean))].slice(0, 40);
	if (unique.length === 0) return [];

	const payload = await api({
		action: "query",
		titles: unique.map((name) => `File:${name}`).join("|"),
		...INFO_PROPS,
	});

	return Object.values(payload?.query?.pages || {});
};

/** Fiche complete d'un fichier Commons (metadonnees, licence et flux video). */
export const getCommonsTitle = async (value) => {
	const name = commonsKey(value).trim();
	if (!name || !VIDEO_EXTENSION.test(name)) throw badRequest("Fichier Wikimedia invalide");

	const pages = await cached(`commons:item:${name}`, config.ttl.medium, () => fetchInfo([name]));
	const page = pages[0];
	const card = page ? commonsCard(page) : null;

	if (!card) throw notFound("Ce fichier n'est plus proposé par Wikimedia Commons");

	const streams = streamsFrom(name, page.videoinfo[0]);
	if (streams.length === 0) throw new ApiError(422, "Aucun fichier vidéo lisible n'est disponible pour ce média");

	return {
		...card,
		subjects: [],
		streams,
		episodes: [],
		kind: "movie",
		embedUrl: null,
		detailsUrl: `${config.commonsApiUrl}/wiki/File:${encodeURIComponent(name.replace(/ /g, "_"))}`,
		sourceLabel: "Wikimedia Commons — domaine public / libre",
	};
};

/** URL distante d'un fichier deja valide (telechargement / reprise). */
/** URL distante d'un fichier deja valide (telechargement / reprise). */
export const commonsRemoteUrl = (identifier, fileName) => {
	const name = commonsKey(identifier).trim();
	const { name: baseName, variant } = splitVariant(fileName);
	if (!name || baseName !== name) return null;

	// URL confirmee par l'API si le fichier a deja ete analyse, sinon reconstruction (repli).
	const known = knownUrls.get(variant ? commonsFileName(name, variant) : commonsFileName(name));
	if (known) return known;

	return variant ? commonsTranscodeUrl(name, variant) : commonsOriginalUrl(name);
};

/**
 * Valide un couple (identifiant Commons, nom de fichier) avant telechargement.
 * Le nom demande doit correspondre exactement au fichier (original ou transcodage existant) :
 * aucune URL fournie par le client n'est utilisee, l'URL finale est recalculee cote serveur.
 */
export const getCommonsFile = async (identifier, fileName) => {
	const name = commonsKey(identifier).trim();
	const { name: baseName, variant } = splitVariant(fileName);

	if (!name || !VIDEO_EXTENSION.test(name)) throw badRequest("Identifiant Wikimedia invalide");
	if (baseName !== name) throw badRequest("Nom de fichier invalide");
	if (variant && !/^[a-z0-9.]{3,40}$/i.test(variant)) throw badRequest("Variante de fichier invalide");

	const pages = await cached(`commons:item:${name}`, config.ttl.medium, () => fetchInfo([name]));
	const info = (pages[0]?.videoinfo || [])[0];
	if (!info) throw notFound("Ce fichier n'existe pas sur Wikimedia Commons");

	const streams = streamsFrom(name, info);
	const stream = streams.find((item) => item.name === (variant ? commonsFileName(name, variant) : name));
	if (!stream) throw notFound("Cette qualité n'est pas disponible pour ce fichier");

	// L'API ne donne pas la taille des transcodages : on la mesure (une requete tres legere)
	// pour que la progression du telechargement soit un vrai pourcentage.
	const size = stream.size || (await probeRemoteSize(stream.url));

	return {
		identifier: commonsFileId(name),
		name: stream.name,
		size,
		format: stream.format,
		url: stream.url,
	};
};
