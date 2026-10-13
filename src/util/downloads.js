/** Aides partagées pour les téléchargements (identifiant source + nom de fichier). */

export const downloadBaseSource = (item) => {
	if (!item) return null;

	// Fiche complète : archiveId (Internet Archive / Wikimedia) ou sourceId.
	const direct = item.archiveId || item.sourceId;
	if (direct) return direct;

	// Carte issue du catalogue interne : seuls `internalId` / `id` sont fournis
	// (la fiche complète ajoute ensuite `sourceId: "internal:<id>"`).
	if (item.internalId) return `internal:${item.internalId}`;

	const id = String(item.id || "");
	return id.startsWith("internal:") ? id : null;
};

export const isInternalSource = (item) => String(downloadBaseSource(item) || "").startsWith("internal:");

export const downloadSourceId = (item, episode = null) => {
	const base = downloadBaseSource(item);
	if (!base) return null;
	if (isInternalSource(item) && episode) {
		return `${base}:${episode.season ?? 1}:${episode.number ?? 1}`;
	}
	return base;
};

export const sanitizeStem = (value) =>
	String(value || "video")
		.replace(/[\\/:*?"<>|]/g, " ")
		.replace(/\s+/g, " ")
		.trim()
		.slice(0, 120) || "video";

export const streamExtension = (stream) => {
	const format = String(stream?.format || "").toLowerCase();
	if (format.includes("webm")) return "webm";
	if (format.includes("mp4") || format.includes("m4v")) return "mp4";
	return "mp4";
};

/**
 * Choisit le flux correspondant le mieux à une qualité demandée.
 *
 * L'API MovieBox ne propose pas les mêmes qualités pour tous les épisodes
 * (une saison récente peut n'exister qu'en 480p, une autre en 360/480/1080).
 * Retomber sur le premier flux faisait télécharger la qualité la plus basse :
 * on prend la qualité exacte si elle existe, sinon la plus proche — et à
 * distance égale, la plus haute (meilleur rendu).
 */
export const qualityHeight = (quality) => {
	const digits = String(quality || "").replace(/[^0-9]/g, "");
	if (!digits) return null;
	const height = Number(digits);
	return height >= 144 ? height : null;
};

export const pickQualityStream = (streams, wanted) => {
	const list = Array.isArray(streams) ? streams.filter(Boolean) : [];
	if (!list.length) return null;

	const target = String(wanted || "").trim().toLowerCase();
	if (target) {
		const exact = list.find((stream) => String(stream.quality || "").toLowerCase() === target);
		if (exact) return exact;
	}

	const wantedHeight = qualityHeight(wanted);
	if (wantedHeight === null) return list[0];

	let best = null;
	let bestDistance = Number.POSITIVE_INFINITY;
	for (const stream of list) {
		const height = qualityHeight(stream.quality);
		if (height === null) continue;

		const distance = Math.abs(height - wantedHeight);
		if (
			distance < bestDistance ||
			(distance === bestDistance && best && height > qualityHeight(best.quality))
		) {
			bestDistance = distance;
			best = stream;
		}
	}

	return best || list[0];
};

const pad = (value) => String(value).padStart(2, "0");

/**
 * Nom du fichier transmis au backend.
 * - Internet Archive : le nom réel du fichier de l'archive (seul accepté par le serveur).
 * - Source MovieBox interne : nom lisible construit avec la qualité (le backend la retrouve dedans).
 */
export const downloadFileName = (item, stream, { episode = null, quality = null } = {}) => {
	if (!isInternalSource(item) && stream?.name) return stream.name;

	const resolved = quality || stream?.quality || "";
	const suffix = episode ? ` S${pad(episode.season ?? 1)}E${pad(episode.number ?? 1)}` : "";
	const qual = resolved && resolved !== "Auto" ? ` ${resolved}` : "";
	return `${sanitizeStem(item?.title)}${suffix}${qual}.${streamExtension(stream)}`;
};
