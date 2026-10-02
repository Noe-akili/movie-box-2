import crypto from "node:crypto";
import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";
import { Readable, Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import { config } from "../config.js";
import { ApiError, badRequest, notFound } from "../lib/errors.js";
import { getFreeFile, freeRemoteUrl, freeSourceLabel } from "./free.js";

/**
 * Gestionnaire de telechargements du backend.
 *
 * Il va chercher les films et les episodes libres de droits sur Internet Archive,
 * les stocke sur le disque du serveur, suit la progression en temps reel, permet de
 * reprendre un transfert interrompu puis de re-servir le fichier en local
 * (lecture hors ligne ou enregistrement sur l'appareil).
 *
 * - manifeste JSON : l'etat survit a un redemarrage du serveur ;
 * - fichier `.part` : la reprise repart de l'octet deja recu (HTTP Range) ;
 * - seules les sources autorisees (Internet Archive) sont acceptees, jamais une URL cliente.
 */
const DIR = config.downloadsDir;
const MANIFEST_FILE = path.join(DIR, "manifest.json");
const PARTIAL_SUFFIX = ".part";

const VIDEO_MIME = {
	".mp4": "video/mp4",
	".m4v": "video/x-m4v",
	".webm": "video/webm",
	".ogv": "video/ogg",
	".mkv": "video/x-matroska",
	".avi": "video/x-msvideo",
	".mov": "video/quicktime",
	".mpg": "video/mpeg",
	".mpeg": "video/mpeg",
};

/** Formats que le navigateur lit directement (les autres restent enregistrables sur l'appareil). */
const BROWSER_PLAYABLE = new Set([".mp4", ".m4v", ".webm", ".ogv"]);

const ACTIVE_STATUSES = new Set(["queued", "downloading"]);

const entries = new Map();
const runners = new Map();
let persistTimer = null;
let running = 0;

const extensionOf = (fileName) => path.extname(String(fileName || "")).toLowerCase();

export const mimeFor = (fileName) => VIDEO_MIME[extensionOf(fileName)] || "application/octet-stream";

export const isPlayableFile = (fileName) => BROWSER_PLAYABLE.has(extensionOf(fileName));

/** Identifiant stable et sur : identifiant d'archive + empreinte du nom de fichier. */
const buildId = (identifier, fileName) =>
	`${String(identifier).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 48) || "media"}-` +
	crypto.createHash("sha1").update(String(fileName)).digest("hex").slice(0, 10);

const remoteUrlFor = (sourceId, fileName) => freeRemoteUrl(sourceId, fileName);

const cleanText = (value, max = 200) => {
	const text = String(value ?? "").replace(/\s+/g, " ").trim();
	return text ? text.slice(0, max) : null;
};

const cleanImage = (value) => {
	const url = String(value || "").trim();
	return /^https?:\/\//i.test(url) ? url.slice(0, 500) : null;
};

const cleanNumber = (value) => {
	const number = Number(value);
	return Number.isFinite(number) && number >= 0 ? Math.floor(number) : null;
};

fs.mkdirSync(DIR, { recursive: true });

const persistNow = () => {
	const tmp = `${MANIFEST_FILE}.tmp`;

	try {
		fs.writeFileSync(tmp, JSON.stringify({ version: 1, entries: [...entries.values()] }));
		fs.renameSync(tmp, MANIFEST_FILE);
	} catch (error) {
		console.warn("[movie-box] manifeste de telechargements non enregistre :", error.message);
	}
};

/** Ecrit le manifeste (au plus une fois par seconde pendant un transfert). */
const persist = (immediate = false) => {
	if (immediate) {
		clearTimeout(persistTimer);
		persistTimer = null;
		persistNow();
		return;
	}

	if (persistTimer) return;
	persistTimer = setTimeout(() => {
		persistTimer = null;
		persistNow();
	}, 900);
};

const filePathOf = (entry) => path.join(DIR, `${entry.id}${entry.extension || extensionOf(entry.fileName)}`);
const partialPathOf = (entry) => `${filePathOf(entry)}${PARTIAL_SUFFIX}`;

const removeFiles = async (entry) => {
	await fsp.rm(filePathOf(entry), { force: true });
	await fsp.rm(partialPathOf(entry), { force: true });
};

const progressOf = (entry) => {
	if (entry.bytesTotal > 0) return Math.min(100, Math.round((entry.bytesReceived / entry.bytesTotal) * 1000) / 10);
	return entry.status === "completed" ? 100 : 0;
};

const etaOf = (entry) => {
	if (entry.status !== "downloading" || !entry.speedBps || entry.bytesTotal <= entry.bytesReceived) return null;
	return Math.max(1, Math.round((entry.bytesTotal - entry.bytesReceived) / entry.speedBps));
};

/** Vue publique d'un telechargement (sans chemin serveur) prete pour l'interface. */
export const publicEntry = (entry) => ({
	id: entry.id,
	sourceId: entry.sourceId,
	fileName: entry.fileName,
	title: entry.title,
	kind: entry.kind,
	seriesTitle: entry.seriesTitle ?? null,
	seasonNumber: entry.seasonNumber ?? null,
	episodeNumber: entry.episodeNumber ?? null,
	episodeName: entry.episodeName ?? null,
	label: entry.label ?? null,
	poster: entry.poster ?? null,
	backdrop: entry.backdrop ?? null,
	quality: entry.quality ?? null,
	format: entry.format ?? null,
	status: entry.status,
	error: entry.error ?? null,
	bytesTotal: entry.bytesTotal || 0,
	bytesReceived: entry.bytesReceived || 0,
	progress: progressOf(entry),
	speedBps: entry.speedBps || 0,
	etaSeconds: etaOf(entry),
	playable: entry.status === "completed" && isPlayableFile(entry.fileName),
	streamUrl: `/api/downloads/${encodeURIComponent(entry.id)}/stream`,
	fileUrl: `/api/downloads/${encodeURIComponent(entry.id)}/file`,
	createdAt: entry.createdAt,
	startedAt: entry.startedAt ?? null,
	updatedAt: entry.updatedAt,
	completedAt: entry.completedAt ?? null,
});

export const listDownloads = () =>
	[...entries.values()]
		.map(publicEntry)
		.sort((a, b) => {
			const rank = (entry) => (ACTIVE_STATUSES.has(entry.status) ? 0 : entry.status === "completed" ? 2 : 1);
			return rank(a) - rank(b) || (b.createdAt || 0) - (a.createdAt || 0);
		});

const entryOrThrow = (id) => {
	const entry = entries.get(String(id || ""));
	if (!entry) throw notFound("Téléchargement introuvable");
	return entry;
};

export const getDownload = (id) => publicEntry(entryOrThrow(id));

/** Acces interne (reserve aux routes) : entree brute avec le chemin du fichier local. */
export const findDownloadEntry = (id) => entryOrThrow(id);

/**
 * Cree (ou renvoie s'il existe deja) un telechargement.
 * Le fichier demande est valide contre les metadonnees Internet Archive avant tout transfert.
 */
export const createDownload = async (payload = {}) => {
	const sourceId = cleanText(payload.sourceId ?? payload.identifier, 240);
	const fileName = cleanText(payload.fileName ?? payload.file, 200);
	const file = await getFreeFile(sourceId, fileName);

	const id = buildId(file.identifier, file.name);
	const existing = entries.get(id);
	if (existing) return publicEntry(existing);

	if (entries.size >= config.downloads.maxQueue) {
		throw new ApiError(
			429,
			`File d'attente pleine (${config.downloads.maxQueue} fichiers) : supprime un téléchargement avant d'en lancer un autre`,
		);
	}

	const size = file.size || cleanNumber(payload.size) || 0;
	if (size > config.downloads.maxBytes) {
		throw new ApiError(
			413,
			`Fichier trop volumineux (${(size / 1024 ** 3).toFixed(1)} Go) : la limite du serveur est de ${(config.downloads.maxBytes / 1024 ** 3).toFixed(1)} Go`,
		);
	}

	const kind = payload.kind === "episode" ? "episode" : "movie";
	const now = Date.now();

	const entry = {
		id,
		sourceId: file.identifier,
		fileName: file.name,
		extension: extensionOf(file.name),
		title: cleanText(payload.title, 200) || file.name,
		kind,
		seriesTitle: kind === "episode" ? cleanText(payload.seriesTitle, 200) : null,
		seasonNumber: kind === "episode" ? cleanNumber(payload.seasonNumber) : null,
		episodeNumber: kind === "episode" ? cleanNumber(payload.episodeNumber) : null,
		episodeName: kind === "episode" ? cleanText(payload.episodeName, 200) : null,
		label: kind === "episode" ? cleanText(payload.label, 40) : null,
		poster: cleanImage(payload.poster),
		backdrop: cleanImage(payload.backdrop),
		quality: cleanText(payload.quality, 20),
		format: cleanText(payload.format, 40) || file.format,
		bytesTotal: size,
		bytesReceived: 0,
		speedBps: 0,
		status: "queued",
		error: null,
		createdAt: now,
		startedAt: null,
		updatedAt: now,
		completedAt: null,
	};

	entries.set(id, entry);
	persist(true);
	enqueue();

	return publicEntry(entry);
};

/** Relance un telechargement en erreur, interrompu ou annule (reprise a l'octet recu). */
export const retryDownload = (id) => {
	const entry = entryOrThrow(id);
	if (entry.status === "downloading") return publicEntry(entry);

	const partial = partialPathOf(entry);
	entry.bytesReceived = fs.existsSync(partial) ? fs.statSync(partial).size : 0;
	entry.speedBps = 0;
	entry.error = null;
	entry.status = "queued";
	entry.updatedAt = Date.now();

	persist(true);
	enqueue();

	return publicEntry(entry);
};

/** Stoppe un transfert en cours en conservant la partie deja recue. */
export const cancelDownload = (id) => {
	const entry = entryOrThrow(id);
	const controller = runners.get(entry.id);

	if (controller && !controller.signal.aborted) {
		entry.status = "canceled";
		entry.updatedAt = Date.now();
		controller.abort();
	} else if (entry.status === "queued") {
		entry.status = "canceled";
		entry.updatedAt = Date.now();
	}

	persist(true);
	return publicEntry(entry);
};

/** Supprime un telechargement et ses fichiers du disque. */
export const removeDownload = async (id) => {
	const entry = entryOrThrow(id);
	const controller = runners.get(entry.id);

	if (controller && !controller.signal.aborted) {
		entry.removed = true;
		controller.abort();
		// Laisse le pipeline se fermer avant de retirer les fichiers.
		await new Promise((resolve) => setTimeout(resolve, 300));
	}

	entries.delete(entry.id);
	runners.delete(entry.id);
	await removeFiles(entry);
	persist(true);

	return { id: entry.id, removed: true };
};

/** Un slot libre = on lance le prochain telechargement en attente. */
const enqueue = () => {
	while (running < config.downloads.concurrency) {
		const next = [...entries.values()].find((entry) => entry.status === "queued" && !entry.removed);
		if (!next) return;

		next.status = "downloading";
		next.startedAt = next.startedAt || Date.now();
		next.updatedAt = Date.now();
		next.error = null;
		running += 1;
		persist(true);

		run(next)
			.catch((error) => console.warn("[movie-box] telechargement interrompu :", error?.message))
			.finally(() => {
				running = Math.max(0, running - 1);
				persist(true);
				enqueue();
			});
	}
};

/** Transfert d'un fichier avec suivi de progression, reprise (HTTP Range) et annulation. */
const run = async (entry) => {
	const finalPath = filePathOf(entry);
	const partial = partialPathOf(entry);
	const controller = new AbortController();
	runners.set(entry.id, controller);

	try {
		let startAt = 0;
		try {
			startAt = (await fsp.stat(partial)).size;
		} catch {
			startAt = 0;
		}
		if (entry.bytesTotal > 0 && startAt >= entry.bytesTotal) startAt = 0;

		const remoteUrl = remoteUrlFor(entry.sourceId, entry.fileName);
		if (!remoteUrl) throw new ApiError(400, "Fichier distant introuvable pour cette source");

		const headers = { "user-agent": "movie-box/1.0" };
		if (startAt > 0) headers.range = `bytes=${startAt}-`;

		const response = await fetch(remoteUrl, {
			headers,
			signal: controller.signal,
			redirect: "follow",
		});

		if (!response.ok) {
			// 404 chez Wikimedia arrive le plus souvent sur un transcodage non (encore) genere :
			// le message doit permettre de comprendre quelle source a refuse.
			throw new ApiError(502, `${freeSourceLabel(entry.sourceId)} a refusé le téléchargement (${response.status})`);
		}
		if (!response.body) throw new ApiError(502, "Réponse sans contenu vidéo");

		const resumed = response.status === 206;
		if (!resumed && startAt > 0) {
			// Le serveur ignore la reprise : on repart du debut.
			await fsp.rm(partial, { force: true });
			startAt = 0;
		}

		const contentLength = Number(response.headers.get("content-length") || 0);
		const total = resumed ? startAt + contentLength : contentLength || entry.bytesTotal || 0;
		if (total > config.downloads.maxBytes) throw new ApiError(413, "Fichier trop volumineux pour le stockage du serveur");

		entry.bytesTotal = total || entry.bytesTotal || 0;
		entry.bytesReceived = startAt;
		entry.updatedAt = Date.now();

		let received = startAt;
		let lastBytes = startAt;
		let lastTick = Date.now();

		const meter = new Transform({
			transform(chunk, _encoding, callback) {
				received += chunk.length;
				entry.bytesReceived = received;
				callback(null, chunk);
			},
		});

		const ticker = setInterval(() => {
			const now = Date.now();
			const elapsed = now - lastTick;
			if (elapsed <= 0) return;

			const instant = ((received - lastBytes) * 1000) / elapsed;
			entry.speedBps = entry.speedBps ? Math.round(entry.speedBps * 0.6 + instant * 0.4) : Math.round(instant);
			entry.updatedAt = now;
			lastBytes = received;
			lastTick = now;
			persist();
		}, 1000);

		try {
			await pipeline(
				Readable.fromWeb(response.body),
				meter,
				fs.createWriteStream(partial, { flags: startAt > 0 ? "a" : "w" }),
			);
		} finally {
			clearInterval(ticker);
		}

		if (entry.removed) return;

		const size = (await fsp.stat(partial)).size;
		if (entry.bytesTotal && size < entry.bytesTotal) {
			throw new ApiError(502, "Téléchargement incomplet (connexion interrompue)");
		}

		await fsp.rename(partial, finalPath);
		entry.bytesTotal = entry.bytesTotal || size;
		entry.bytesReceived = size;
		entry.speedBps = 0;
		entry.status = "completed";
		entry.error = null;
		entry.completedAt = Date.now();
		entry.updatedAt = entry.completedAt;
	} catch (error) {
		entry.speedBps = 0;
		entry.updatedAt = Date.now();

		if (controller.signal.aborted) {
			if (entry.status !== "canceled") entry.status = "paused";
		} else {
			entry.status = "error";
			entry.error = error?.message || "Téléchargement interrompu";
		}
	} finally {
		runners.delete(entry.id);
	}
};

/** Etat global : compteurs, volume stocke et espace disque restant. */
export const downloadsSummary = async () => {
	const items = [...entries.values()];
	const completed = items.filter((entry) => entry.status === "completed");
	const active = items.filter((entry) => ACTIVE_STATUSES.has(entry.status));
	const count = (status) => items.filter((entry) => entry.status === status).length;

	let freeBytes = null;
	try {
		const stats = await fsp.statfs(DIR);
		freeBytes = Number(stats.bavail) * Number(stats.bsize);
	} catch {
		freeBytes = null;
	}

	return {
		total: items.length,
		completed: completed.length,
		active: active.length,
		queued: count("queued"),
		errors: count("error"),
		paused: count("paused") + count("canceled"),
		completedBytes: completed.reduce((sum, entry) => sum + (entry.bytesTotal || 0), 0),
		activeBytes: active.reduce((sum, entry) => sum + (entry.bytesReceived || 0), 0),
		maxBytes: config.downloads.maxBytes,
		maxQueue: config.downloads.maxQueue,
		concurrency: config.downloads.concurrency,
		freeBytes,
	};
};

/** Sert un fichier deja telecharge (lecture inline ou enregistrement sur l'appareil). */
export const sendDownloadFile = (entry, req, res, { download = false } = {}) => {
	const filePath = filePathOf(entry);

	if (entry.status !== "completed" || !fs.existsSync(filePath)) {
		throw new ApiError(409, "Ce téléchargement n'est pas encore terminé");
	}

	const stats = fs.statSync(filePath);
	const headers = {
		"content-type": mimeFor(entry.fileName),
		"accept-ranges": "bytes",
		"cache-control": "private, max-age=0",
		"content-disposition": `${download ? "attachment" : "inline"}; filename*=UTF-8''${encodeURIComponent(entry.fileName)}`,
	};

	const match = /^bytes=(\d*)-(\d*)$/.exec(String(req.headers.range || "").trim());

	if (match && (match[1] || match[2])) {
		const start = match[1] ? Number(match[1]) : Math.max(0, stats.size - Number(match[2]));
		const end = match[1] && match[2] ? Math.min(Number(match[2]), stats.size - 1) : stats.size - 1;

		if (!Number.isFinite(start) || !Number.isFinite(end) || start >= stats.size || start > end) {
			res.status(416).set("content-range", `bytes */${stats.size}`).end();
			return;
		}

		res.writeHead(206, {
			...headers,
			"content-range": `bytes ${start}-${end}/${stats.size}`,
			"content-length": end - start + 1,
		});
		fs.createReadStream(filePath, { start, end }).pipe(res);
		return;
	}

	res.writeHead(200, { ...headers, "content-length": stats.size });
	fs.createReadStream(filePath).pipe(res);
};

/** Recharge le manifeste au demarrage, puis relance les transferts inacheves. */
const restore = () => {
	let stored = [];

	try {
		stored = JSON.parse(fs.readFileSync(MANIFEST_FILE, "utf8"))?.entries || [];
	} catch {
		return;
	}

	if (!Array.isArray(stored)) return;

	stored.forEach((raw) => {
		const id = cleanText(raw?.id, 80);
		const fileName = cleanText(raw?.fileName, 200);
		const sourceId = cleanText(raw?.sourceId, 240);
		if (!id || !fileName || !sourceId) return;

		const entry = {
			...raw,
			id,
			sourceId,
			fileName,
			extension: extensionOf(fileName),
			title: cleanText(raw?.title, 200) || fileName,
			status: ACTIVE_STATUSES.has(raw?.status) ? "queued" : raw?.status || "paused",
			speedBps: 0,
			error: null,
		};

		if (entry.status !== "completed") {
			const partial = partialPathOf(entry);
			entry.bytesReceived = fs.existsSync(partial) ? fs.statSync(partial).size : 0;
		}

		entries.set(id, entry);
	});

	const pending = [...entries.values()].filter((entry) => entry.status === "queued");
	if (pending.length) console.log(`[movie-box] ${pending.length} telechargement(s) repris apres redemarrage`);
};

export { DIR as downloadsDir };

restore();
enqueue();
