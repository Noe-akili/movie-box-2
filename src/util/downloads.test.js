/**
 * Tests des aides de téléchargement (film ET épisode) — `node --test`.
 * Ces fonctions sont pures (aucune dépendance Vue) : on teste le contrat envoyé
 * au backend, notamment l'identifiant de source `internal:<id>:<saison>:<épisode>`
 * que `get_free_file` (Rust) re-découpe en saison/épisode.
 */

import test from "node:test";
import assert from "node:assert/strict";

import {
	downloadBaseSource,
	downloadFileName,
	downloadSourceId,
	isInternalSource,
	pickQualityStream,
	qualityHeight,
	sanitizeStem,
	streamExtension,
} from "./downloads.js";

test("film archive : identifiant = archiveId", () => {
	const movie = { mediaType: "archive", archiveId: "nosferatu_1922", title: "Nosferatu" };
	assert.equal(downloadBaseSource(movie), "nosferatu_1922");
	assert.equal(isInternalSource(movie), false);
	assert.equal(downloadSourceId(movie), "nosferatu_1922");
});

test("film archive : le nom réel du fichier est transmis tel quel", () => {
	const movie = { mediaType: "archive", archiveId: "nosferatu_1922", title: "Nosferatu" };
	const stream = { name: "Nosferatu_1922_512kb.mp4", quality: "SD", format: "MP4", size: 1024 };
	assert.equal(downloadFileName(movie, stream), "Nosferatu_1922_512kb.mp4");
});

test("film interne : nom construit avec la qualité", () => {
	const movie = { mediaType: "internal", internalId: "123", title: "Un film : sous-titre", streams: [] };
	const stream = { quality: "1080p", format: "MP4" };
	assert.equal(isInternalSource(movie), true);
	assert.equal(downloadSourceId(movie), "internal:123");
	assert.equal(downloadFileName(movie, stream), "Un film sous-titre 1080p.mp4");
});

test("film interne : qualité « Auto » retirée du nom de fichier", () => {
	const movie = { mediaType: "internal", internalId: "123", title: "Film auto" };
	assert.equal(downloadFileName(movie, { quality: "Auto", format: "MP4" }, { quality: "Auto" }), "Film auto.mp4");
});

test("épisode interne : identifiant porteur de la saison et de l'épisode", () => {
	const series = { mediaType: "internal", internalId: "456", kind: "series", title: "Série" };
	const episode = { season: 2, number: 7 };
	assert.equal(downloadSourceId(series, episode), "internal:456:2:7");
	assert.equal(downloadFileName(series, { quality: "720p", format: "MP4" }, { episode }), "Série S02E07 720p.mp4");
});

test("épisode interne : saison/épisode par défaut à 1", () => {
	const series = { mediaType: "internal", internalId: "456", kind: "series", title: "Série" };
	assert.equal(downloadSourceId(series, {}), "internal:456:1:1");
	assert.equal(downloadFileName(series, null, { episode: {}, quality: "480p" }), "Série S01E01 480p.mp4");
});

test("épisode archive : nom du fichier réel, pas de suffixe ajouté", () => {
	const series = { mediaType: "archive", archiveId: "serie_libre", kind: "series", title: "Série libre" };
	const episode = { season: 1, number: 3, streams: [{ name: "serie_s01e03.webm", quality: "SD", format: "WebM" }] };
	const stream = episode.streams[0];

	assert.equal(downloadSourceId(series, episode), "serie_libre");
	assert.equal(downloadFileName(series, stream, { episode }), "serie_s01e03.webm");
});

test("extension : HLS/DASH retombent sur mp4, webm conservé", () => {
	assert.equal(streamExtension({ format: "MP4" }), "mp4");
	assert.equal(streamExtension({ format: "WebM" }), "webm");
	assert.equal(streamExtension({ format: "HLS" }), "mp4");
	assert.equal(streamExtension(undefined), "mp4");
});

test("nom de fichier nettoyé des caractères interdits", () => {
	assert.equal(sanitizeStem("Titre: avec / des * caractères ?"), "Titre avec des caractères");
	assert.equal(sanitizeStem(""), "video");
});

test("contrat backend : « internal:<id>:<saison>:<épisode> » se re-découpe", () => {
	const sourceId = downloadSourceId({ internalId: "456", kind: "series" }, { season: 2, number: 7 });
	const [, id, season, episode] = /^internal:([^:]+):(\d+):(\d+)$/.exec(sourceId);
	assert.equal(id, "456");
	assert.equal(Number(season), 2);
	assert.equal(Number(episode), 7);
});

test("qualité : la valeur exacte est choisie", () => {
	const streams = [
		{ quality: "360p", size: 1 },
		{ quality: "480p", size: 2 },
		{ quality: "1080p", size: 3 },
	];
	assert.equal(pickQualityStream(streams, "480p").quality, "480p");
	assert.equal(pickQualityStream(streams, "1080p").quality, "1080p");
});

test("qualité absente : on prend la plus proche, jamais la plus basse", () => {
	// Cas réel de la saison 8 de The Rookie : seul le 480p existe.
	const only480 = [{ quality: "480p", size: 42 }];
	assert.equal(pickQualityStream(only480, "1080p").quality, "480p");

	// 720p demandé, épisode en 360/480/1080 : 480 est le plus proche (240 < 360).
	const mixed = [
		{ quality: "360p", size: 1 },
		{ quality: "480p", size: 2 },
		{ quality: "1080p", size: 3 },
	];
	assert.equal(pickQualityStream(mixed, "720p").quality, "480p");

	// 360p demandé mais épisode en 480 minimum : 480 (et non la plus basse d'une
	// autre liste, ni un repli silencieux sur la 360p d'un autre épisode).
	assert.equal(pickQualityStream([{ quality: "480p" }, { quality: "1080p" }], "360p").quality, "480p");

	// À distance égale (600 demandé, 480 et 720 disponibles) : la plus haute.
	const tie = [
		{ quality: "480p", size: 1 },
		{ quality: "720p", size: 2 },
	];
	assert.equal(pickQualityStream(tie, "600p").quality, "720p");
});

test("qualité : comparaison insensible à la casse, liste vide ou inconnue", () => {
	const streams = [{ quality: "720P" }];
	assert.equal(pickQualityStream(streams, "720p").quality, "720P");
	assert.equal(pickQualityStream([], "1080p"), null);
	assert.equal(pickQualityStream([{ quality: "Auto" }], "1080p").quality, "Auto");
	assert.equal(qualityHeight("1080p"), 1080);
	assert.equal(qualityHeight("Auto"), null);
});
