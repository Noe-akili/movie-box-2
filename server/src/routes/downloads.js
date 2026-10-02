import { Router } from "express";
import { asyncHandler } from "../lib/errors.js";
import {
	cancelDownload,
	createDownload,
	downloadsSummary,
	findDownloadEntry,
	getDownload,
	listDownloads,
	removeDownload,
	retryDownload,
	sendDownloadFile,
} from "../services/downloads.js";

/**
 * Routes des telechargements.
 *
 * GET    /api/downloads              liste + progression + etat du stockage
 * POST   /api/downloads              lance le telechargement d'un film ou d'un episode
 * GET    /api/downloads/:id          detail d'un telechargement
 * POST   /api/downloads/:id/retry    relance / reprend un transfert
 * POST   /api/downloads/:id/cancel   met en pause un transfert en cours
 * DELETE /api/downloads/:id          supprime le telechargement et son fichier
 * GET    /api/downloads/:id/stream   lecture du fichier local (video, supporte Range)
 * GET    /api/downloads/:id/file     enregistrement du fichier sur l'appareil
 */
export const downloads = Router();

// La progression doit rester fraiche : on annule le cache court applique a /api.
downloads.use((req, res, next) => {
	res.setHeader("cache-control", "no-store");
	next();
});

downloads.get(
	"/",
	asyncHandler(async (req, res) => {
		res.json({ items: listDownloads(), summary: await downloadsSummary() });
	}),
);

downloads.post(
	"/",
	asyncHandler(async (req, res) => {
		res.status(201).json(await createDownload(req.body || {}));
	}),
);

downloads.get("/:id", (req, res) => {
	res.json(getDownload(req.params.id));
});

downloads.post("/:id/retry", (req, res) => {
	res.json(retryDownload(req.params.id));
});

downloads.post("/:id/cancel", (req, res) => {
	res.json(cancelDownload(req.params.id));
});

downloads.delete(
	"/:id",
	asyncHandler(async (req, res) => {
		res.json(await removeDownload(req.params.id));
	}),
);

downloads.get("/:id/stream", (req, res) => {
	sendDownloadFile(findDownloadEntry(req.params.id), req, res, { download: false });
});

downloads.get("/:id/file", (req, res) => {
	sendDownloadFile(findDownloadEntry(req.params.id), req, res, { download: true });
});
