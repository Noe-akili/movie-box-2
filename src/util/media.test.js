/** Tests de classification des types (film / série / gratuit) — `node --test`. */

import test from "node:test";
import assert from "node:assert/strict";

import { isFree, isSeriesTitle, mediaTypeLabel, seasonCountOf } from "./media.js";

test("film TMDB : ni série ni gratuit", () => {
	const movie = { mediaType: "movie", tmdbId: 123, title: "Film", runtime: 120, seasons: null };
	assert.equal(isFree(movie), false);
	assert.equal(isSeriesTitle(movie), false);
	assert.equal(mediaTypeLabel(movie), "FILM");
	assert.equal(seasonCountOf(movie), null);
});

test("série TMDB : détectée par mediaType", () => {
	const series = { mediaType: "tv", tmdbId: 456, title: "Série", seasons: [1, 2, 3] };
	assert.equal(isSeriesTitle(series), true);
	assert.equal(mediaTypeLabel(series), "SÉRIE");
	assert.equal(seasonCountOf(series), 3);
});

test("fiche interne : `kind` de l'API fait foi", () => {
	const movie = { mediaType: "internal", kind: "movie", sourceId: "internal:123", streams: [{ quality: "720p" }] };
	const series = {
		mediaType: "internal",
		kind: "series",
		sourceId: "internal:456",
		streams: [{ quality: "720p" }],
		episodes: [{ season: 1, number: 1, streams: [] }],
	};

	assert.equal(isFree(movie), true);
	assert.equal(mediaTypeLabel(movie), "FILM");
	assert.equal(isSeriesTitle(movie), false);

	assert.equal(isFree(series), true);
	assert.equal(isSeriesTitle(series), true);
	assert.equal(mediaTypeLabel(series), "SÉRIE");
});

test("carte interne : plusieurs saisons annoncent une série", () => {
	const card = { mediaType: "internal", internalId: "789", title: "Solo", seasons: 2, episodes: 24 };
	assert.equal(isSeriesTitle(card), true);
	assert.equal(mediaTypeLabel(card), "SÉRIE");
	assert.equal(seasonCountOf(card), 2);
});

test("archive : épisodes détectés => série (même sans `kind`)", () => {
	const archive = {
		mediaType: "archive",
		archiveId: "serie_libre",
		title: "Série libre",
		streams: [{ name: "s01e01.mp4" }],
		episodes: [
			{ season: 1, number: 1, streams: [{ name: "s01e01.mp4" }] },
			{ season: 1, number: 2, streams: [{ name: "s01e02.mp4" }] },
		],
	};

	assert.equal(isFree(archive), true);
	assert.equal(isSeriesTitle(archive), true);
	assert.equal(mediaTypeLabel(archive), "SÉRIE");
});

test("archive à un seul épisode mais `kind: series` reste une série", () => {
	const archive = {
		mediaType: "archive",
		archiveId: "unique",
		kind: "series",
		episodes: [{ season: 1, number: 1, streams: [{ name: "e01.mp4" }] }],
	};

	assert.equal(isSeriesTitle(archive), true);
});

test("sans épisode ni saison : film gratuit", () => {
	const archive = { mediaType: "archive", archiveId: "court", episodes: [], streams: [{ name: "film.mp4" }] };
	assert.equal(isSeriesTitle(archive), false);
	assert.equal(mediaTypeLabel(archive), "FILM");
});

test("carte interne : `subjectType` 2 annonce une série même sans compteur", () => {
	const series = { mediaType: "internal", internalId: "42", title: "Série cachée", subjectType: 2 };
	assert.equal(isSeriesTitle(series), true);
	assert.equal(mediaTypeLabel(series), "SÉRIE");
});

test("carte interne : `subjectType` 1 reste un film même sans compteur", () => {
	const movie = { mediaType: "internal", internalId: "43", title: "Film", subjectType: 1 };
	assert.equal(isSeriesTitle(movie), false);
	assert.equal(mediaTypeLabel(movie), "FILM");
});

test("carte interne : compteur d'épisodes > 1 => série", () => {
	const series = { mediaType: "internal", internalId: "44", title: "Saison unique", episodes: 10 };
	assert.equal(isSeriesTitle(series), true);
});

test("une série gratuite garde son type après passage par le catalogue", () => {
	// Reproduit le bug : la carte ne porte que `subjectType`, la modale de
	// téléchargement doit quand même savoir qu'il s'agit d'une série.
	const card = { mediaType: "internal", internalId: "45", title: "Série gratuite", subjectType: 2, seasons: 1 };
	assert.equal(isFree(card), true);
	assert.equal(isSeriesTitle(card), true);
	assert.equal(mediaTypeLabel(card), "SÉRIE");
});
