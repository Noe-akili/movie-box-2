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

export const qualityLabel = (item) => (isFree(item) ? "GRATUIT" : item.badge || (item.mediaType === "tv" ? "SÉRIE" : "FILM"));
