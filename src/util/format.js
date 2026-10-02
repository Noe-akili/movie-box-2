/** Formats d'affichage partages par toute l'application (locale fr). */

export const formatRuntime = (minutes) => {
	const total = Number(minutes);
	if (!Number.isFinite(total) || total <= 0) return null;

	const hours = Math.floor(total / 60);
	const rest = total % 60;

	if (hours === 0) return `${rest} min`;
	if (rest === 0) return `${hours} h`;
	return `${hours} h ${String(rest).padStart(2, "0")}`;
};

export const formatCount = (value) => {
	const number = Number(value);
	if (!Number.isFinite(number) || number <= 0) return null;
	if (number >= 1_000_000) return `${(number / 1_000_000).toFixed(1).replace(".", ",")} M`;
	if (number >= 1_000) return `${(number / 1_000).toFixed(1).replace(".", ",")} k`;
	return String(number);
};

export const formatDate = (value) => {
	if (!value) return null;

	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return null;

	return date.toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" });
};

export const formatSize = (bytes) => {
	const size = Number(bytes);
	if (!Number.isFinite(size) || size <= 0) return null;

	const units = ["o", "Ko", "Mo", "Go"];
	const index = Math.min(units.length - 1, Math.floor(Math.log(size) / Math.log(1024)));

	return `${(size / 1024 ** index).toFixed(index === 0 ? 0 : 1).replace(".", ",")} ${units[index]}`;
};

export const metaLine = (parts) => parts.filter(Boolean).join(" • ");

/** Debit de telechargement (o/s, Ko/s, Mo/s...). */
export const formatSpeed = (bytesPerSecond) => {
	const speed = Number(bytesPerSecond);
	if (!Number.isFinite(speed) || speed <= 0) return null;
	return `${formatSize(speed)}/s`;
};

/** Duree restante a partir de secondes (ex. « 2 min 05 », « 1 h 12 »). */
export const formatEta = (seconds) => {
	const total = Number(seconds);
	if (!Number.isFinite(total) || total <= 0) return null;

	if (total < 60) return `${Math.round(total)} s`;

	const minutes = Math.floor(total / 60);
	if (minutes < 60) return `${minutes} min ${String(Math.round(total % 60)).padStart(2, "0")}`;

	const hours = Math.floor(minutes / 60);
	return `${hours} h ${String(minutes % 60).padStart(2, "0")}`;
};
