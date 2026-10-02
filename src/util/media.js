/** Aide au routage/normalisation des cartes venant de l'API (TMDB ou Internet Archive). */

export const mediaKey = (item) => (item?.mediaType === "archive" ? item.archiveId : item?.tmdbId);

export const detailRoute = (item) => ({
	name: "detail",
	params: { mediaType: item.mediaType, id: String(mediaKey(item)) },
});

export const watchRoute = (item) => ({
	name: "watch",
	params: { mediaType: item.mediaType, id: String(mediaKey(item)) },
});

export const isFree = (item) => item?.mediaType === "archive" || item?.source === "archive";

export const qualityLabel = (item) => (isFree(item) ? "GRATUIT" : item.badge || (item.mediaType === "tv" ? "SÉRIE" : "FILM"));
