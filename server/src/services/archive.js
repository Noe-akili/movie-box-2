import { config } from "../config.js";
import { cached } from "../lib/cache.js";
import { ApiError, badRequest, notFound } from "../lib/errors.js";
import { buildUrl, fetchJson } from "../lib/http.js";

const SEARCH_URL = `${config.archiveBaseUrl}/advancedsearch.php`;
const FIELDS = [
	"identifier",
	"title",
	"year",
	"date",
	"downloads",
	"description",
	"creator",
	"licenseurl",
	"collection",
	"language",
	"runtime",
];

const VIDEO_EXTENSIONS = /\.(mp4|m4v|webm|ogv|mkv|avi|mpeg|mpg|mov)$/i;
const FORMAT_PRIORITY = [
	"h.264 IA",
	"h.264 HD",
	"HiRes MPEG4",
	"h.264",
	"MPEG4",
	"512Kb MPEG4",
	"MPEG2",
	"WebM",
	"Ogg Video",
	"Matroska",
];

const sanitizeTerm = (value) =>
	String(value || "")
		.replace(/[^\p{L}\p{N}\s'-]/gu, " ")
		.replace(/\s+/g, " ")
		.trim()
		.slice(0, 80);

const stripHtml = (value) =>
	String(Array.isArray(value) ? value.join(" ") : value || "")
		.replace(/<[^>]*>/g, " ")
		.replace(/&amp;/g, "&")
		.replace(/&quot;/g, '"')
		.replace(/&#39;/g, "'")
		.replace(/&nbsp;/g, " ")
		.replace(/\s+/g, " ")
		.trim();

const firstValue = (value) => (Array.isArray(value) ? value.find(Boolean) : value) || null;

const parseYear = (value) => {
	const match = String(firstValue(value) || "").match(/(1[89]\d{2}|20\d{2})/);
	return match ? Number(match[1]) : null;
};

const parseRuntimeMinutes = (value) => {
	const raw = String(firstValue(value) || "").trim();
	if (!raw) return null;

	const clock = raw.match(/^(\d{1,2}):(\d{2})(?::(\d{2}))?$/);
	if (clock) {
		const [, a, b] = clock;
		return Number(a) * 60 + Number(b);
	}

	const hours = raw.match(/(\d+)\s*h(?:eures?)?\s*(\d+)?/i);
	if (hours) return Number(hours[1]) * 60 + Number(hours[2] || 0);

	const minutes = raw.match(/(\d+)\s*(min|minutes?|mn)/i) || raw.match(/^(\d+)$/);
	return minutes ? Number(minutes[1]) : null;
};

const SORTS = {
	popular: "downloads desc",
	recent: "date desc",
	oldest: "date asc",
	title: "title asc",
	rating: "avg_rating desc",
};

export const resolveArchiveSort = (value) => SORTS[value] || SORTS.popular;

/**
 * Filtres de droits proposes a l'utilisateur.
 * Internet Archive declare les droits de chaque item dans `licenseurl` (domaine public « mark/zero »
 * ou l'une des licences Creative Commons). Le filtre par defaut ne laisse passer que ces œuvres :
 * la garantie « domaine public / libre » est donc appliquee cote source, avant tout affichage ou transfert.
 */
export const ARCHIVE_LICENSES = [
	{
		id: "free",
		label: "Domaine public & Creative Commons",
		clause: "(licenseurl:*creativecommons* OR licenseurl:*publicdomain*)",
	},
	{ id: "publicdomain", label: "Domaine public uniquement", clause: "licenseurl:*publicdomain*" },
	{
		id: "cc",
		label: "Creative Commons (hors domaine public)",
		clause: "(licenseurl:*creativecommons* AND NOT licenseurl:*publicdomain*)",
	},
	{ id: "all", label: "Toutes les archives (droits non verifies)", clause: "" },
];

export const DEFAULT_ARCHIVE_LICENSE = "free";

/**
 * Collections Archive.org proposees dans l'application.
 * Toutes sont filtrees par defaut sur les droits declares (domaine public / Creative Commons) :
 * `license` permet d'affiner ou d'elargir ce filtre depuis la page « Films gratuits ».
 */
export const ARCHIVE_COLLECTIONS = [
	{ id: "feature_films", label: "Longs-métrages libres" },
	{ id: "moviesandfilms", label: "Films populaires du web" },
	{ id: "classic_tv", label: "Séries TV classiques" },
	{ id: "television", label: "Télévision & émissions" },
	{ id: "Film_Noir", label: "Cinéma noir" },
	{ id: "SciFi_Horror", label: "Science-fiction & horreur" },
	{ id: "silent_films", label: "Films muets" },
	{ id: "animationandcartoons", label: "Animation" },
	{ id: "classic_cartoons", label: "Dessins animés" },
	{ id: "short_films", label: "Courts-métrages" },
	{ id: "prelinger", label: "Archives Prelinger" },
	{ id: "opensource_movies", label: "Vidéos libres (open source)" },
];

export const DEFAULT_ARCHIVE_COLLECTION = "feature_films";

export const resolveArchiveLicense = (value) =>
	ARCHIVE_LICENSES.find((license) => license.id === value) ||
	ARCHIVE_LICENSES.find((license) => license.id === DEFAULT_ARCHIVE_LICENSE);

/** Libelle lisible d'une licence a partir de son URL (affichage sur les fiches). */
const licenseLabelOf = (url) => {
	const value = String(firstValue(url) || "").toLowerCase();
	if (!value) return null;
	if (value.includes("publicdomain/zero")) return "CC0 — domaine public";
	if (value.includes("publicdomain/mark") || value.includes("/licenses/publicdomain/")) return "Domaine public";
	if (value.includes("/by-nc-sa/")) return "CC BY-NC-SA";
	if (value.includes("/by-nc-nd/")) return "CC BY-NC-ND";
	if (value.includes("/by-nc/")) return "CC BY-NC";
	if (value.includes("/by-sa/")) return "CC BY-SA";
	if (value.includes("/by-nd/")) return "CC BY-ND";
	if (value.includes("/by/")) return "CC BY";

	return "Licence libre (Creative Commons)";
};

const buildSearchQuery = ({ q, collection, license }) => {
	const clauses = ["mediatype:(movies)"];
	if (collection) clauses.push(`collection:(${collection})`);

	// Le filtre de droits s'applique a toutes les requetes : collections mises en avant comme recherche libre.
	const { clause } = resolveArchiveLicense(license);
	if (clause) clauses.push(clause);

	const term = sanitizeTerm(q);
	if (term) clauses.push(`(title:"${term}" OR description:"${term}")`);
	return clauses.join(" AND ");
};

export const archiveCard = (doc) => {
	const identifier = doc?.identifier;
	if (!identifier) return null;

	const description = stripHtml(doc.description);

	return {
		id: `archive-${identifier}`,
		mediaType: "archive",
		archiveId: identifier,
		source: "archive",
		badge: "GRATUIT",
		title: String(firstValue(doc.title) || identifier),
		overview: description,
		poster: `${config.archiveBaseUrl}/services/img/${identifier}`,
		thumb: `${config.archiveBaseUrl}/services/img/${identifier}`,
		backdrop: `${config.archiveBaseUrl}/services/img/${identifier}`,
		year: parseYear(doc.year || doc.date),
		rating: null,
		downloads: Number(doc.downloads || 0),
		creator: firstValue(doc.creator) || null,
		license: firstValue(doc.licenseurl) || null,
		licenseLabel: licenseLabelOf(doc.licenseurl),
		collection: firstValue(doc.collection) || null,
		language: firstValue(doc.language) || null,
		runtime: parseRuntimeMinutes(doc.runtime),
		genres: [],
		genreIds: [],
	};
};


export const searchArchive = async ({
	q = "",
	collection = "",
	license = DEFAULT_ARCHIVE_LICENSE,
	page = 1,
	rows = 24,
	sort = "popular",
} = {}) => {
	const safePage = Math.max(1, Number(page) || 1);
	const safeRows = Math.min(60, Math.max(1, Number(rows) || 24));
	const activeLicense = resolveArchiveLicense(license);
	const cacheKey = `archive:search:${q}:${collection}:${activeLicense.id}:${safePage}:${safeRows}:${sort}`;

	return cached(cacheKey, config.ttl.medium, async () => {
		const url = buildUrl(SEARCH_URL, {
			q: buildSearchQuery({ q, collection, license: activeLicense.id }),
			rows: safeRows,
			page: safePage,
			output: "json",
		});

		// L'API attend des parametres repetes (fl[]=..., sort[]=...) : ajoutes apres buildUrl.
		const params = FIELDS.map((field) => `fl%5B%5D=${field}`).join("&");
		const finalUrl = `${url}&${params}&sort%5B%5D=${encodeURIComponent(resolveArchiveSort(sort))}`;

		const payload = await fetchJson(finalUrl, { timeoutMs: 25_000, retries: 2 });
		const response = payload?.response || {};

		return {
			page: safePage,
			rows: safeRows,
			license: activeLicense.id,
			total: response.numFound || 0,
			hasMore: safePage * safeRows < (response.numFound || 0),
			items: (response.docs || []).map(archiveCard).filter(Boolean),
		};
	});
};

/** Bruit de qualite a ignorer pour regrouper les fichiers d'un meme episode. */
const QUALITY_NOISE = /\b(512\s?kb|1\s?mb|hq|hd|sd|4k|1080p|720p|480p|360p|full|remastered|restored|ia)\b/gi;

const baseName = (name) => String(name || "").replace(/\.[a-z0-9]{2,5}$/i, "");

const normalizedName = (name) =>
	baseName(name)
		.replace(QUALITY_NOISE, " ")
		.replace(/[^\p{L}\p{N}]+/gu, " ")
		.trim()
		.toLowerCase();

/** Motifs de numerotation d'episode : chaque entree indique les groupes saison / numero. */
const EPISODE_PATTERNS = [
	{ pattern: /\bS(\d{1,2})[\s._-]*E(\d{1,3})\b/i, season: 1, number: 2 },
	{ pattern: /\b(\d{1,2})\s?x(\d{1,3})\b/i, season: 1, number: 2 },
	{
		pattern:
			/\b(?:ep|episode|part|pt|chapter|cap[ií]tulo|capitulo|folge|episodio|epis[oó]dio)[\s._-]*(\d{1,3})\b/i,
		number: 1,
	},
	{ pattern: /[-\u2013\u2014]\s*(\d{1,3})(?!\d)\s*[-\u2013\u2014]/, number: 1 },
	{ pattern: /^\s*(\d{1,3})(?!\d)[\s._-]+[^\d]/, number: 1 },
];

const SEASON_PATTERN = /\bs(?:aison|eason)?[\s._-]*(\d{1,2})\b/i;

/** Detecte saison et numero d'episode dans un nom de fichier (S01E02, 1x02, « Ep 3 », « - 001 - »…). */
const episodeFromName = (name) => {
	const base = baseName(name);

	for (const { pattern, season: seasonGroup, number: numberGroup } of EPISODE_PATTERNS) {
		const match = base.match(pattern);
		if (!match) continue;

		const number = Number(match[numberGroup]);
		if (!Number.isFinite(number) || number <= 0) continue;

		const season = seasonGroup ? Number(match[seasonGroup]) : null;

		if (Number.isFinite(season) && season > 0) return { season, number };

		// Nombreux items Internet Archive rangent les episodes dans un dossier « Season 3 ».
		const folderSeason = base.match(SEASON_PATTERN);
		const fallback = folderSeason ? Number(folderSeason[1]) : NaN;

		return { season: Number.isFinite(fallback) && fallback > 0 ? fallback : null, number };
	}

	return null;
};

const episodeLabel = (episode) =>
	episode.season
		? `S${String(episode.season).padStart(2, "0")}E${String(episode.number).padStart(2, "0")}`
		: `Épisode ${episode.number}`;

/** Nom lisible d'un episode (sans dossier, ni extension, ni bruit de qualite). */
const episodeName = (name) => {
	const leaf = String(name || "").split("/").pop() || "";

	return (
		baseName(leaf)
			.replace(QUALITY_NOISE, " ")
			.replace(/[\s._-]+/g, " ")
			.trim() || baseName(leaf)
	);
};

/**
 * Analyse les fichiers video d'une archive : une entree = un fichier video.
 * `groupKey` permet de retrouver les variantes de qualite d'un meme episode ou d'un meme film.
 */
const analyzeFiles = (identifier, files = []) =>
	files
		.filter((file) => file?.name && VIDEO_EXTENSIONS.test(file.name))
		.map((file) => {
			const format = String(file.format || "");
			const priority = FORMAT_PRIORITY.findIndex((value) => value.toLowerCase() === format.toLowerCase());
			const isMp4 = /\.(mp4|m4v)$/i.test(file.name);
			const quality =
				/h\.264 IA|HD|HiRes/i.test(format) || /1080|720/.test(file.name)
					? "HD"
					: /512Kb|MPEG4|MPEG2|WebM|Ogg/i.test(format) || isMp4
						? "SD"
						: "Original";
			const episode = episodeFromName(file.name);

			return {
				name: file.name,
				format: format || "Vidéo",
				size: Number(file.size || 0),
				quality,
				url: `${config.archiveBaseUrl}/download/${encodeURIComponent(identifier)}/${encodeURIComponent(file.name)}`,
				score: (isMp4 ? 0 : 10) + (priority < 0 ? 20 : priority),
				groupKey: episode
					? `episode:${episode.season ?? "x"}:${episode.number}`
					: `file:${normalizedName(file.name)}`,
				episode,
			};
		})
		.sort((a, b) => a.score - b.score || b.size - a.size);

/** Retire les champs internes utilises pour le tri et le regroupement. */
const stripInternal = ({ score, groupKey, episode, ...stream }) => stream;

/**
 * Construit la bibliotheque lisible d'une archive :
 * - `streams` : toutes les qualites disponibles (lecture / telechargement) ;
 * - `episodes` : episodes detectes (une archive multi-fichiers = une saison ou une serie) ;
 * - `kind` : « series » des qu'au moins deux episodes sont identifies, sinon « movie ».
 */
const buildLibrary = (identifier, files = []) => {
	const analyzed = analyzeFiles(identifier, files);
	const streams = analyzed.map(stripInternal);

	if (streams.length === 0) return { streams, episodes: [], kind: "movie" };

	const groups = new Map();
	for (const item of analyzed) {
		const group = groups.get(item.groupKey);
		if (!group) {
			groups.set(item.groupKey, { best: item, items: [item] });
			continue;
		}

		group.items.push(item);
		if (item.score < group.best.score) group.best = item;
	}

	const episodes = [...groups.values()]
		.filter((group) => group.best.episode)
		.map((group) => ({
			season: group.best.episode.season,
			number: group.best.episode.number,
			label: episodeLabel(group.best.episode),
			...stripInternal(group.best),
			name: episodeName(group.best.name),
			streams: group.items.map(stripInternal),
		}))
		.sort((a, b) => (a.season ?? 0) - (b.season ?? 0) || a.number - b.number);

	return { streams, episodes, kind: episodes.length > 1 ? "series" : "movie" };
};

export const getArchiveTitle = async (identifier) => {
	if (!identifier) throw notFound("Film introuvable");

	const payload = await cached(`archive:item:${identifier}`, config.ttl.medium, () =>
		fetchJson(`${config.archiveBaseUrl}/metadata/${encodeURIComponent(identifier)}`, { timeoutMs: 15_000 }),
	);

	if (!payload || payload.is_dark || payload.metadata?.is_dark === "true") {
		throw notFound("Cette vidéo n'est plus proposée par Internet Archive");
	}

	const metadata = payload.metadata || {};
	const { streams, episodes, kind } = buildLibrary(identifier, payload.files || []);

	if (streams.length === 0) {
		throw new ApiError(422, "Aucun fichier vidéo lisible n'est disponible pour cette archive");
	}

	const card = archiveCard({
		identifier,
		title: metadata.title,
		year: metadata.year || metadata.date,
		downloads: metadata.downloads,
		description: metadata.description,
		creator: metadata.creator,
		licenseurl: metadata.licenseurl,
		collection: metadata.collection,
		language: metadata.language,
		runtime: metadata.runtime,
	});

	return {
		...card,
		overview: card.overview || stripHtml(metadata.description),
		subjects: (Array.isArray(metadata.subject) ? metadata.subject : [metadata.subject])
			.filter(Boolean)
			.slice(0, 8)
			.map(String),
		streams,
		episodes,
		kind,
		embedUrl: `${config.archiveBaseUrl}/embed/${encodeURIComponent(identifier)}`,
		detailsUrl: `${config.archiveBaseUrl}/details/${encodeURIComponent(identifier)}`,
		sourceLabel: "Internet Archive — domaine public / libre",
	};
};

/**
 * Valide un fichier video d'une archive (identifiant + nom de fichier) avant tout telechargement.
 * Le nom du fichier doit exister tel quel dans les metadonnees de l'item : aucune URL client n'est acceptee.
 */
export const getArchiveFile = async (identifier, fileName) => {
	const cleanId = String(identifier || "").trim();
	const cleanName = String(fileName || "").trim();

	if (!/^[\w.\-]{1,120}$/.test(cleanId)) throw badRequest("Identifiant d'archive invalide");
	if (!cleanName || cleanName.length > 200 || cleanName.includes("/") || cleanName.includes("\\")) {
		throw badRequest("Nom de fichier invalide");
	}

	const payload = await cached(`archive:item:${cleanId}`, config.ttl.medium, () =>
		fetchJson(`${config.archiveBaseUrl}/metadata/${encodeURIComponent(cleanId)}`, { timeoutMs: 15_000 }),
	);

	const file = (payload?.files || []).find((entry) => entry?.name === cleanName);

	if (!file) throw notFound("Ce fichier n'existe pas dans cette archive");
	if (!VIDEO_EXTENSIONS.test(cleanName)) throw badRequest("Ce fichier n'est pas une vidéo lisible");

	return {
		identifier: cleanId,
		name: cleanName,
		size: Number(file.size || 0),
		format: String(file.format || "Vidéo"),
		url: `${config.archiveBaseUrl}/download/${encodeURIComponent(cleanId)}/${encodeURIComponent(cleanName)}`,
	};
};

export const getRelatedArchive = async ({ identifier, collection, rows = 12 } = {}) => {
	const result = await searchArchive({ collection, rows: rows + 1, page: 1, sort: "popular" });
	return {
		...result,
		items: result.items.filter((item) => item.archiveId !== identifier).slice(0, rows),
	};
};
