import { config, hasTmdbKey } from "../config.js";
import { cached } from "../lib/cache.js";
import { ApiError } from "../lib/errors.js";
import { buildUrl, fetchJson } from "../lib/http.js";

export const SUPPORTED_LANGS = ["fr-FR", "en-US", "es-ES", "pt-BR", "ar-SA"];
export const DEFAULT_LANG = "fr-FR";
export const MEDIA_TYPES = ["movie", "tv"];

export const resolveLang = (value) => (SUPPORTED_LANGS.includes(value) ? value : DEFAULT_LANG);

export const isMediaType = (value) => MEDIA_TYPES.includes(value);

/** Appel TMDB authentifie + cache memoire (cle = URL complete). */
export const tmdbRequest = async (path, params = {}, { ttlMs = config.ttl.short, lang = DEFAULT_LANG } = {}) => {
	if (!hasTmdbKey()) {
		throw new ApiError(503, "Cle TMDB absente : ajoute TMDB_API_KEY (ou VITE_API_KEY) dans le fichier .env");
	}

	const url = buildUrl(`${config.tmdbBaseUrl}${path}`, {
		api_key: config.tmdbApiKey,
		language: resolveLang(lang),
		...params,
	});

	return cached(`tmdb:${url}`, ttlMs, () => fetchJson(url));
};

export const imageUrl = (path, size = "w500") =>
	path ? `${config.tmdbImageBaseUrl}/${size}${path}` : null;

const yearFrom = (value) => {
	const year = Number(String(value || "").slice(0, 4));
	return Number.isFinite(year) && year > 1800 ? year : null;
};

/** Transforme une entree TMDB (film ou serie) en "carte" normalisee pour le front. */
export const cardFromTmdb = (item, fallbackType = "movie") => {
	if (!item) return null;

	const mediaType = item.media_type === "person"
		? null
		: isMediaType(item.media_type)
			? item.media_type
			: fallbackType;
	if (!mediaType) return null;

	const title = item.title || item.name || item.original_title || item.original_name;
	if (!title || !item.id) return null;

	const date = mediaType === "tv" ? item.first_air_date : item.release_date;

	return {
		id: `${mediaType}-${item.id}`,
		mediaType,
		tmdbId: item.id,
		source: "tmdb",
		badge: mediaType === "tv" ? "SÉRIE" : "FILM",
		title,
		originalTitle: item.original_title || item.original_name || null,
		overview: item.overview || "",
		poster: imageUrl(item.poster_path, "w500"),
		thumb: imageUrl(item.poster_path, "w342"),
		backdrop: imageUrl(item.backdrop_path, "w1280"),
		year: yearFrom(date),
		releaseDate: date || null,
		rating: typeof item.vote_average === "number" ? Number(item.vote_average.toFixed(1)) : null,
		votes: item.vote_count ?? null,
		popularity: item.popularity ?? 0,
		genreIds: item.genre_ids || (item.genres || []).map((genre) => genre.id),
		genres: (item.genres || []).map((genre) => genre.name),
		runtime: item.runtime ?? null,
		seasons: item.number_of_seasons ?? null,
		episodes: item.number_of_episodes ?? null,
		originalLanguage: item.original_language || null,
	};
};

export const cardsFromResponse = (response, fallbackType = "movie") =>
	(response?.results || []).map((item) => cardFromTmdb(item, fallbackType)).filter(Boolean);

export const findTrailer = (videos = []) => {
	const youtube = videos.filter((video) => video.site === "YouTube" && video.key);
	const byType = (type) => youtube.find((video) => video.type === type);
	const trailer = byType("Trailer") || byType("Teaser") || youtube[0];

	return trailer
		? {
			key: trailer.key,
			name: trailer.name,
			embedUrl: `https://www.youtube.com/embed/${trailer.key}?autoplay=1&rel=0`,
			watchUrl: `https://www.youtube.com/watch?v=${trailer.key}`,
		}
		: null;
};

/** Liste des plateformes LEGALES de diffusion, avec un drapeau "gratuit". */
export const providersFromResponse = (response, region = config.watchRegion) => {
	const regionData = response?.results?.[region] || response?.results?.US || null;
	if (!regionData) return { region, link: null, groups: [] };

	const groups = [
		{ key: "free", label: "Gratuit" },
		{ key: "ads", label: "Gratuit avec publicite" },
		{ key: "flatrate", label: "Inclus dans un abonnement" },
		{ key: "rent", label: "Location" },
		{ key: "buy", label: "Achat" },
	]
		.map((group) => ({
			label: group.label,
			free: group.key === "free" || group.key === "ads",
			items: (regionData[group.key] || []).map((provider) => ({
				id: provider.provider_id,
				name: provider.provider_name,
				logo: imageUrl(provider.logo_path, "w92"),
				free: group.key === "free" || group.key === "ads",
			})),
		}))
		.filter((group) => group.items.length > 0);

	return { region, link: regionData.link || null, groups };
};

export const creditsFromResponse = (credits) => {
	const crew = credits?.crew || [];
	const cast = (credits?.cast || []).slice(0, 16).map((person) => ({
		id: person.id,
		name: person.name,
		character: person.character || null,
		photo: imageUrl(person.profile_path, "w185"),
	}));

	return {
		cast,
		directors: crew.filter((member) => member.job === "Director").map((member) => member.name),
		writers: [...new Set(
			crew
				.filter((member) => ["Writer", "Screenplay", "Story"].includes(member.job))
				.map((member) => member.name),
		)].slice(0, 4),
	};
};
