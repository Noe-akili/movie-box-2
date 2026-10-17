/** Aide au routage/normalisation des cartes venant de l'API (MovieBox, TMDB ou Internet Archive). */

export const mediaKey = (item) => {
	if (item?.mediaType === "archive") return item.archiveId;
	if (item?.mediaType === "internal") return item.id || item.internalId;
	return item?.tmdbId;
};

export const detailRoute = (item) => ({
	name: "detail",
	params: { mediaType: item.mediaType, id: String(mediaKey(item)) },
});

export const watchRoute = (item) => ({
	name: "watch",
	params: { mediaType: item.mediaType, id: String(mediaKey(item)) },
});

export const isFree = (item) =>
	item?.mediaType === "archive" ||
	item?.mediaType === "internal" ||
	item?.source === "archive" ||
	item?.source === "internal";


/**
 * Type réel du titre, déduit des données fournies par l'API (pas seulement de
 * `mediaType`) : le backend Movie Box envoie `kind: "movie" | "series"` sur la
 * fiche, les cartes de séries internes portent un compte de saisons, et une
 * archive libre regroupe ses fichiers en `episodes`. On croise ces trois
 * signaux pour ne jamais confondre une série gratuite avec un film.
 */
export const isSeriesTitle = (item) => {
	if (!item) return false;

	// `kind` fixé par le backend (fiches complètes) est prioritaire sur tout.
	const kind = String(item.kind || "").toLowerCase();
	if (kind === "series") return true;
	if (kind === "movie") return false;

	if (item.mediaType === "tv") return true;

	// Cartes de la source interne : `subjectType` (1 = film, 2 = série) est le
	// signal fourni par l'API. Sans lui, une série dont le catalogue ne renvoie
	// ni saison ni compteur d'épisodes était prise pour un film (modale de
	// téléchargement faussée). On le lit AVANT les replis numériques.
	const subjectType = Number(item.subjectType);
	if (subjectType === 2) return true;
	if (subjectType === 1) return false;

	const episodes = Array.isArray(item.episodes) ? item.episodes.length : Number(item.episodes) || 0;
	if (episodes > 1) return true;

	const seasons = Array.isArray(item.seasons) ? item.seasons.length : Number(item.seasons) || 0;
	return seasons > 1;
};

/** Libellé court du type (badge des affiches et de l'en-tête). */
export const mediaTypeLabel = (item) => {
	if (isFree(item)) {
		return isSeriesTitle(item) ? "SÉRIE" : "FILM";
	}
	return item.mediaType === "tv" ? "SÉRIE" : item.badge || "FILM";
};

/** Nombre de saisons quand il est connu (TMDB : tableau ou compte). */
export const seasonCountOf = (item) => {
	const value = Array.isArray(item?.seasons) ? item.seasons.length : Number(item?.seasons);
	return Number.isFinite(value) && value > 0 ? value : null;
};

export const qualityLabel = (item) => (isFree(item) ? "GRATUIT" : item.badge || mediaTypeLabel(item));

