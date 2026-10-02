import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(currentDir, "..", "..");

/**
 * Charge un fichier .env sans dependance externe.
 * Les variables de l'environnement shell restent prioritaires.
 */
const readEnvFile = (filePath) => {
	if (!fs.existsSync(filePath)) return {};

	return fs.readFileSync(filePath, "utf8").split(/\r?\n/).reduce((acc, line) => {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith("#")) return acc;

		const separator = trimmed.indexOf("=");
		if (separator === -1) return acc;

		const key = trimmed.slice(0, separator).trim();
		const value = trimmed.slice(separator + 1).trim().replace(/^['"]|['"]$/g, "");
		if (key) acc[key] = value;
		return acc;
	}, {});
};

const fileEnv = {
	...readEnvFile(path.join(rootDir, ".env")),
	...readEnvFile(path.join(currentDir, "..", ".env")),
};
const env = { ...fileEnv, ...process.env };

const toList = (value) =>
	String(value || "*")
		.split(",")
		.map((item) => item.trim())
		.filter(Boolean);

export const config = {
	rootDir,
	distDir: path.join(rootDir, "dist"),
	port: Number(env.PORT || env.SERVER_PORT || 5174),
	// La cle TMDB est lue depuis TMDB_API_KEY, sinon depuis la cle deja presente dans le .env du projet.
	tmdbApiKey: env.TMDB_API_KEY || env.VITE_API_KEY || "",
	tmdbBaseUrl: env.TMDB_BASE_URL || "https://api.themoviedb.org/3",
	tmdbImageBaseUrl: "https://image.tmdb.org/t/p",
	archiveBaseUrl: env.ARCHIVE_BASE_URL || "https://archive.org",
	// Wikimedia Commons : deuxieme source de films libres (licences verifiees par la communaute).
	commonsApiUrl: env.COMMONS_API_URL || "https://commons.wikimedia.org",
	commonsUploadUrl: env.COMMONS_UPLOAD_URL || "https://upload.wikimedia.org",
	// Region utilisee pour "ou regarder legalement" (plateformes locales + gratuites avec pub).
	watchRegion: (env.WATCH_REGION || "FR").toUpperCase(),
	allowedOrigins: toList(env.ALLOWED_ORIGINS),
	// Telechargements : les fichiers sont stockes sur le disque du serveur puis re-servis en local.
	downloadsDir: env.DOWNLOADS_DIR
		? path.resolve(rootDir, env.DOWNLOADS_DIR)
		: path.join(rootDir, "server", "downloads"),
	downloads: {
		concurrency: Math.min(4, Math.max(1, Number(env.DOWNLOAD_CONCURRENCY || 2))),
		maxBytes: Number(env.DOWNLOAD_MAX_BYTES || 8 * 1024 * 1024 * 1024),
		maxQueue: Number(env.DOWNLOAD_MAX_QUEUE || 60),
	},
	rateLimit: {
		windowMs: Number(env.RATE_LIMIT_WINDOW_MS || 60_000),
		max: Number(env.RATE_LIMIT_MAX || 240),
	},
	ttl: {
		short: 5 * 60 * 1000,
		medium: 30 * 60 * 1000,
		long: 6 * 60 * 60 * 1000,
	},
};

export const hasTmdbKey = () => Boolean(config.tmdbApiKey);
