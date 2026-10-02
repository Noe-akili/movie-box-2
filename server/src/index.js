import fs from "node:fs";
import path from "node:path";
import cors from "cors";
import express from "express";
import { config } from "./config.js";
import { ApiError } from "./lib/errors.js";
import { rateLimit } from "./middleware/rateLimit.js";
import { api } from "./routes/api.js";

const app = express();

app.set("trust proxy", true);
app.disable("x-powered-by");

app.use(
	cors({
		origin: config.allowedOrigins.includes("*") ? true : config.allowedOrigins,
		maxAge: 3600,
	}),
);
app.use(express.json({ limit: "256kb" }));
app.use((req, res, next) => {
	res.setHeader("x-content-type-options", "nosniff");
	res.setHeader("referrer-policy", "no-referrer-when-downgrade");
	next();
});

// API : rate limit + cache navigateur court (les donnees TMDB bougent lentement).
app.use("/api", rateLimit);
app.use("/api", (req, res, next) => {
	// Les telechargements renvoient de la progression : jamais de cache navigateur.
	if (req.path.startsWith("/downloads")) {
		res.setHeader("cache-control", "no-store");
		return next();
	}

	res.setHeader("cache-control", "public, max-age=60");
	next();
});
app.use("/api", api);

// Frontend compile (npm run build) servi par le meme serveur Node.
const indexHtml = path.join(config.distDir, "index.html");
const hasBuild = fs.existsSync(indexHtml);

if (hasBuild) {
	app.use(express.static(config.distDir, { maxAge: "1h", index: false }));
} else {
	app.get("/", (req, res) => {
		res.json({
			name: "Movie Box API",
			status: "ok",
			frontend: "Le dossier dist/ est absent : lance « npm run dev » (frontend Vite) ou « npm run build » pour servir l'interface ici.",
			endpoints: [
				"GET /api/health",
				"GET /api/home",
				"GET /api/genres?type=movie|tv",
				"GET /api/search?q=",
				"GET /api/discover/movie|tv?genre=&sort=&page=&language=",
				"GET /api/free?collection=&q=&page=",
				"GET /api/title/:mediaType/:id",
				"GET /api/title/tv/:id/season/:n",
				"GET /api/downloads",
				"POST /api/downloads",
			],
		});
	});
}

// Fallback SPA (Vue Router en mode history) pour tout ce qui n'est pas /api.
app.use((req, res, next) => {
	if (req.method !== "GET" || req.path.startsWith("/api") || !hasBuild) return next();
	return res.sendFile(indexHtml);
});

app.use((error, req, res, next) => {
	const status = error instanceof ApiError ? error.status : 500;
	if (status >= 500) console.error("[movie-box]", error);

	res.status(status).json({
		error: {
			status,
			message: error?.message || "Erreur interne du serveur",
		},
	});
});

const server = app.listen(config.port, () => {
	console.log(`\n  Movie Box API  ->  http://localhost:${config.port}/api/health`);
	console.log(`  TMDB configuré : ${config.tmdbApiKey ? "oui" : "NON (ajoute TMDB_API_KEY dans .env)"}`);
	console.log(`  Frontend servi : ${hasBuild ? "oui (dossier dist/)" : "non (utilise npm run dev)"}`);
	console.log(`  Téléchargements : ${config.downloadsDir}\n`);
});

process.on("unhandledRejection", (reason) => console.error("[movie-box] promesse rejetée :", reason));
process.on("SIGTERM", () => server.close(() => process.exit(0)));
process.on("SIGINT", () => server.close(() => process.exit(0)));

export default app;
