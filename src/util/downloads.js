/** Aides partagées pour les téléchargements (identifiant source + nom de fichier). */

export const downloadBaseSource = (item) => item?.archiveId || item?.sourceId || null;

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
