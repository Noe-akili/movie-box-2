import { Router } from "express";
import { config } from "../config.js";
import { cacheStats } from "../lib/cache.js";
import { ApiError, asyncHandler, badRequest, notFound } from "../lib/errors.js";
import {
	cardFromTmdb,
	cardsFromResponse,
	creditsFromResponse,
	findTrailer,
	imageUrl,
	isMediaType,
	providersFromResponse,
	resolveLang,
	tmdbRequest,
} from "../services/tmdb.js";
import {
	ARCHIVE_LICENSES,
	DEFAULT_ARCHIVE_LICENSE,
	getArchiveTitle,
	getRelatedArchive,
	resolveArchiveLicense,
	searchArchive,
} from "../services/archive.js";
import {
	DEFAULT_FREE_SOURCE,
	FREE_SOURCES,
	defaultFreeCollection,
	freeCollections,
	freeLicenses,
	getFreeTitle,
	resolveFreeSource,
	searchFree,
} from "../services/free.js";
import { searchCommons } from "../services/commons.js";
import { buildLegalLinks, legalInfo } from "../services/providers.js";
import { downloads } from "./downloads.js";

export const api = Router();

const parsePage = (value) => Math.min(500, Math.max(1, Number.parseInt(value, 10) || 1));
const langOf = (req) => resolveLang(req.query.lang);
const regionOf = (req) => String(req.query.region || config.watchRegion).toUpperCase().slice(0, 2);
const licenseOf = (req) => resolveArchiveLicense(req.query.license).id;
const sourceOf = (req) => resolveFreeSource(req.query.source).id;

/**
 * Execute des taches asynchrones par paquets.
 * Archive.org n'aime pas les rafales : on limite les requetes simultanees tout en evitant
 * de payer la latence de chaque rail l'un apres l'autre (9 rails sur l'accueil).
 */
const mapWithLimit = async (items, limit, task) => {
	const results = new Array(items.length).fill(null);
	let cursor = 0;

	const worker = async () => {
		while (cursor < items.length) {
			const index = cursor;
			cursor += 1;

			try {
				results[index] = await task(items[index]);
			} catch (error) {
				console.warn(`[movie-box] rail archive « ${items[index].id} » indisponible : ${error.message}`);
				results[index] = null;
			}
		}
	};

	await Promise.all(Array.from({ length: Math.max(1, Math.min(limit, items.length)) }, worker));
	return results;
};

/** Rails TMDB de la page d'accueil (le catalogue mondial complet passe par /discover). */
const TMDB_RAILS = [
	{
		id: "trending-week",
		title: "Tendances de la semaine",
		path: "/trending/all/week",
		href: "/movies",
		ttl: config.ttl.short,
	},
	{
		id: "popular-movies",
		title: "Films populaires",
		path: "/movie/popular",
		href: "/movies",
	},
	{
		id: "popular-tv",
		title: "Séries à ne pas manquer",
		path: "/tv/popular",
		mediaType: "tv",
		href: "/series",
	},
	{
		id: "anime",
		title: "Anime & animation japonaise",
		path: "/discover/tv",
		params: { with_genres: 16, with_original_language: "ja", sort_by: "popularity.desc" },
		mediaType: "tv",
		href: "/series?genre=16",
	},
	{
		id: "top-rated",
		title: "Les mieux notés de tous les temps",
		path: "/movie/top_rated",
		ttl: config.ttl.long,
		href: "/movies?sort=vote_average.desc",
	},
	{
		id: "action",
		title: "Action & aventure",
		path: "/discover/movie",
		params: { with_genres: 28, sort_by: "popularity.desc" },
		href: "/movies?genre=28",
	},
	{
		id: "korea",
		title: "K-dramas & cinéma coréen",
		path: "/discover/tv",
		params: { with_original_language: "ko", sort_by: "popularity.desc" },
		mediaType: "tv",
		href: "/series?language=ko",
	},
	{
		id: "french",
		title: "Cinéma français",
		path: "/discover/movie",
		params: { with_original_language: "fr", sort_by: "popularity.desc" },
		href: "/movies?language=fr",
	},
	{
		id: "india",
		title: "Bollywood & cinéma indien",
		path: "/discover/movie",
		params: { with_original_language: "hi", sort_by: "popularity.desc" },
		href: "/movies?language=hi",
	},
	{
		id: "horror",
		title: "Frissons garantis",
		path: "/discover/movie",
		params: { with_genres: 27, sort_by: "popularity.desc" },
		href: "/movies?genre=27",
	},
];

/**
 * Rails de films gratuits et légaux (Internet Archive : domaine public / libre).
 * `license` filtre les droits déclarés par l'Archive ; sans précision, « free » s'applique
 * (domaine public + Creative Commons), donc aucune œuvre aux droits non vérifiés.
 */
const ARCHIVE_RAILS = [
	{ id: "free-features", title: "Films gratuits — domaine public", collection: "feature_films" },
	{ id: "free-series", title: "Séries TV libres de droits", collection: "classic_tv" },
	{ id: "free-noir", title: "Polars & cinéma noir", collection: "Film_Noir" },
	{ id: "free-scifi", title: "Science-fiction rétro", collection: "SciFi_Horror" },
	{ id: "free-silent", title: "Chefs-d'œuvre du cinéma muet", collection: "silent_films" },
	{ id: "free-cartoons", title: "Animation & dessins animés", collection: "animationandcartoons" },
	{ id: "free-shorts", title: "Courts-métrages libres", collection: "short_films" },
	{ id: "free-prelinger", title: "Archives Prelinger (1950-1980)", collection: "prelinger" },
	{ id: "free-cc-modern", title: "Films sous Creative Commons", collection: "feature_films", license: "cc" },
];

api.get("/health", (req, res) => {
	res.json({
		status: "ok",
		uptime: Math.round(process.uptime()),
		cache: cacheStats(),
		tmdbConfigured: Boolean(config.tmdbApiKey),
		region: config.watchRegion,
	});
});

api.get("/legal", (req, res) => {
	res.json(legalInfo());
});

api.get(
	"/genres",
	asyncHandler(async (req, res) => {
		const type = req.query.type === "tv" ? "tv" : "movie";
		const lang = langOf(req);
		const payload = await tmdbRequest(`/genre/${type}/list`, {}, { ttlMs: config.ttl.long, lang });
		res.json({ type, genres: payload?.genres || [] });
	}),
);

api.get(
	"/home",
	asyncHandler(async (req, res) => {
		const lang = langOf(req);

		const [, ...railResults] = await Promise.allSettled([
			tmdbRequest("/trending/all/week", {}, { ttlMs: config.ttl.short, lang }),
			...TMDB_RAILS.map((rail) =>
				tmdbRequest(rail.path, rail.params || {}, { ttlMs: rail.ttl || config.ttl.short, lang }),
			),
		]);

		// Archive.org limite les requetes simultanees : les rails gratuits sont charges par paquets de 3.
		const archiveResults = await mapWithLimit(ARCHIVE_RAILS, 3, (rail) =>
			searchArchive({
				collection: rail.collection,
				license: rail.license || DEFAULT_ARCHIVE_LICENSE,
				rows: 20,
				sort: "popular",
			}),
		);

		const rails = [];
		let hero = [];

		railResults.forEach((result, index) => {
			if (result.status !== "fulfilled") {
				console.warn(`[movie-box] rail ${TMDB_RAILS[index].id} indisponible : ${result.reason?.message}`);
				return;
			}

			const definition = TMDB_RAILS[index];
			const items = cardsFromResponse(result.value, definition.mediaType || "movie").filter(Boolean);
			if (items.length === 0) return;

			rails.push({
				id: definition.id,
				title: definition.title,
				kind: "tmdb",
				href: definition.href || null,
				items,
			});

			if (definition.id === "trending-week") {
				hero = items.filter((item) => item.backdrop).slice(0, 8);
			}
		});

		// Rail Wikimedia Commons : un seul appel (mis en cache), licences verifiees par la communaute.
		const commonsRail = await searchCommons({ rows: 20 }).catch((error) => {
			console.warn(`[movie-box] rail Wikimedia indisponible : ${error.message}`);
			return null;
		});

		ARCHIVE_RAILS.forEach((definition, index) => {
			const result = archiveResults[index];
			if (!result?.items?.length) return;

			const license = definition.license || DEFAULT_ARCHIVE_LICENSE;

			rails.push({
				id: definition.id,
				title: definition.title,
				kind: "archive",
				href: `/free?collection=${definition.collection}${
					license !== DEFAULT_ARCHIVE_LICENSE ? `&license=${license}` : ""
				}`,
				items: result.items,
			});
		});

		if (commonsRail?.items?.length) {
			rails.push({
				id: "free-commons",
				title: "Domaine public — Wikimedia Commons",
				kind: "archive",
				href: "/free?source=commons",
				items: commonsRail.items,
			});
		}

		if (hero.length === 0) {
			hero = rails.flatMap((rail) => rail.items).filter((item) => item.backdrop).slice(0, 8);
		}

		res.json({
			lang,
			region: config.watchRegion,
			hero,
			rails,
			generatedAt: new Date().toISOString(),
		});
	}),
);


/**
 * Collections Archive.org proposees dans l'application : elles vivent dans le service
 * (`services/archive.js`) pour que la couche « sources gratuites » puisse les agreger.
 */
export { ARCHIVE_COLLECTIONS } from "../services/archive.js";

const SORT_WHITELIST = [
	"popularity.desc",
	"popularity.asc",
	"vote_average.desc",
	"vote_count.desc",
	"primary_release_date.desc",
	"primary_release_date.asc",
	"revenue.desc",
];

api.get(
	"/search",
	asyncHandler(async (req, res) => {
		const query = String(req.query.q || "").trim();
		if (!query) throw badRequest("Paramètre « q » requis");

		const page = parsePage(req.query.page);
		const lang = langOf(req);
		const type = req.query.type === "movie" || req.query.type === "tv" ? req.query.type : "all";
		const year = /^\d{4}$/.test(String(req.query.year || "")) ? req.query.year : undefined;

		const tmdbPaths = type === "movie" ? ["/search/movie"] : type === "tv" ? ["/search/tv"] : ["/search/movie", "/search/tv"];
		const [tmdbSettled, freeSettled, commonsSettled] = await Promise.allSettled([
			Promise.all(
				tmdbPaths.map((path) =>
					tmdbRequest(
						path,
						{ query, page, include_adult: false, ...(year ? { year } : {}) },
						{ ttlMs: config.ttl.short, lang },
					),
				),
			),
			searchFree({
				source: sourceOf(req),
				q: query,
				rows: 20,
				page,
				sort: "popular",
				license: licenseOf(req),
			}),
			// La recherche globale couvre les deux catalogues libres, quel que soit l'onglet choisi.
			sourceOf(req) === "archive" ? searchCommons({ q: query, rows: 12, page }) : Promise.resolve(null),
		]);

		const results = tmdbSettled.status === "fulfilled"
			? tmdbSettled.value
				.flatMap((payload, index) => cardsFromResponse(payload, tmdbPaths[index] === "/search/tv" ? "tv" : "movie"))
				.sort((a, b) => (b.popularity || 0) - (a.popularity || 0))
			: [];

		const primaryFree =
			freeSettled.status === "fulfilled"
				? freeSettled.value
				: { source: sourceOf(req), license: DEFAULT_ARCHIVE_LICENSE, total: 0, hasMore: false, items: [] };
		const commonsFree = commonsSettled?.status === "fulfilled" ? commonsSettled.value : null;
		const knownIds = new Set(primaryFree.items.map((item) => item.archiveId));
		const freeItems = commonsFree
			? [...primaryFree.items, ...commonsFree.items.filter((item) => !knownIds.has(item.archiveId))]
			: primaryFree.items;

		const free = {
			source: sourceOf(req),
			license: primaryFree.license,
			total: primaryFree.total,
			hasMore: primaryFree.hasMore,
			commons: commonsFree ? commonsFree.items.length : 0,
			items: freeItems,
		};

		res.json({
			query,
			page,
			type,
			count: results.length,
			results,
			free,
		});
	}),
);

api.get(
	"/discover/:mediaType",
	asyncHandler(async (req, res) => {
		const { mediaType } = req.params;
		if (!isMediaType(mediaType)) throw badRequest("Type de média invalide (movie ou tv)");

		const lang = langOf(req);
		const page = parsePage(req.query.page);
		const sort = SORT_WHITELIST.includes(String(req.query.sort)) ? req.query.sort : "popularity.desc";
		const genre = String(req.query.genre || "").replace(/[^0-9|]/g, "");
		const language = /^[a-z]{2}$/.test(String(req.query.language || "")) ? req.query.language : undefined;
		const country = String(req.query.country || "").replace(/[^A-Za-z|]/g, "").toUpperCase() || undefined;
		const year = /^\d{4}$/.test(String(req.query.year || "")) ? req.query.year : undefined;

		const params = {
			page,
			sort_by: sort,
			include_adult: false,
			...(genre ? { with_genres: genre } : {}),
			...(language ? { with_original_language: language } : {}),
			...(country ? { with_origin_country: country } : {}),
			...(year
				? mediaType === "tv"
					? { first_air_date_year: year }
					: { primary_release_year: year }
				: {}),
			...(sort.startsWith("vote_average") ? { "vote_count.gte": 120 } : {}),
		};

		const payload = await tmdbRequest(`/discover/${mediaType}`, params, { ttlMs: config.ttl.short, lang });

		res.json({
			mediaType,
			page: payload?.page || page,
			totalPages: Math.min(payload?.total_pages || 1, 500),
			total: payload?.total_results || 0,
			hasMore: (payload?.page || page) < Math.min(payload?.total_pages || 1, 500),
			items: cardsFromResponse(payload, mediaType),
		});
	}),
);

api.get(
	"/free",
	asyncHandler(async (req, res) => {
		const source = sourceOf(req);
		const collections = freeCollections(source);
		const collection = collections.some((item) => item.id === req.query.collection)
			? String(req.query.collection)
			: defaultFreeCollection(source);
		const sort = ["popular", "recent", "oldest", "title", "rating"].includes(String(req.query.sort))
			? String(req.query.sort)
			: "popular";
		const license = licenseOf(req);

		const result = await searchFree({
			source,
			q: String(req.query.q || "").trim(),
			collection,
			license,
			page: parsePage(req.query.page),
			rows: Math.min(48, Math.max(6, Number.parseInt(req.query.rows, 10) || 30)),
			sort,
		});

		res.json({
			...result,
			source,
			collection,
			sort,
			license,
			collections,
			licenses: freeLicenses(source),
			sources: FREE_SOURCES,
			legal: legalInfo(),
		});
	}),
);

// Telechargements locaux (films et episodes libres de droits stockes par le backend).
api.use("/downloads", downloads);

api.get(
	"/title/tv/:id/season/:seasonNumber",
	asyncHandler(async (req, res) => {
		const id = String(req.params.id).replace(/\D/g, "");
		const seasonNumber = Number.parseInt(req.params.seasonNumber, 10);

		if (!id || !Number.isFinite(seasonNumber)) throw badRequest("Identifiant de série ou numéro de saison invalide");

		const lang = langOf(req);
		const payload = await tmdbRequest(`/tv/${id}/season/${seasonNumber}`, {}, { ttlMs: config.ttl.medium, lang });

		res.json({
			seasonNumber,
			name: payload?.name || `Saison ${seasonNumber}`,
			overview: payload?.overview || "",
			poster: imageUrl(payload?.poster_path, "w500"),
			airDate: payload?.air_date || null,
			episodes: (payload?.episodes || []).map((episode) => ({
				id: episode.id,
				episodeNumber: episode.episode_number,
				name: episode.name || `Épisode ${episode.episode_number}`,
				overview: episode.overview || "",
				still: imageUrl(episode.still_path, "w500"),
				airDate: episode.air_date || null,
				runtime: episode.runtime ?? null,
				rating: typeof episode.vote_average === "number" ? Number(episode.vote_average.toFixed(1)) : null,
			})),
		});
	}),
);

api.get(
	"/title/:mediaType/:id",
	asyncHandler(async (req, res) => {
		const { mediaType } = req.params;
		const rawId = decodeURIComponent(String(req.params.id));
		const lang = langOf(req);
		const region = regionOf(req);

		// 1. Films libres de droits : lecture directe dans l'application (Internet Archive ou Wikimedia).
		if (mediaType === "archive") {
			const title = await getFreeTitle(rawId);
			const related = await getRelatedArchive({
				identifier: rawId,
				collection: title.collection || "feature_films",
				rows: 12,
			}).catch(() => ({ items: [] }));

			return res.json({
				...title,
				playable: true,
				player: "html5",
				related: related.items,
				externalLinks: [
					{
						id: "details",
						name: title.source === "commons" ? "Page Wikimedia Commons" : "Page Internet Archive",
						description: "Source, fichiers et licence de l'œuvre",
						url: title.detailsUrl,
						free: true,
					},
				],
				legal: legalInfo(),
			});
		}

		if (!isMediaType(mediaType)) throw badRequest("Type de média invalide (movie, tv ou archive)");

		const tmdbId = rawId.replace(/\D/g, "");
		if (!tmdbId) throw badRequest("Identifiant TMDB invalide");

		const append = [
			"credits",
			"videos",
			"images",
			"recommendations",
			"similar",
			"watch/providers",
			...(mediaType === "tv" ? ["content_ratings", "external_ids"] : ["release_dates"]),
		].join(",");

		const request = (language) =>
			tmdbRequest(
				`/${mediaType}/${tmdbId}`,
				{
					append_to_response: append,
					watch_region: region,
					include_image_language: `${language.slice(0, 2)},en,null`,
				},
				{ ttlMs: config.ttl.medium, lang: language },
			);

		let data = await request(lang);

		// TMDB ne fournit pas toujours de synopsis traduit : repli sur l'anglais.
		if (!data?.overview && lang !== "en-US") {
			const fallback = await request("en-US").catch(() => null);
			if (fallback?.overview) data = { ...data, overview: fallback.overview };
		}

		const card = cardFromTmdb(data, mediaType);
		if (!card) throw notFound("Ce titre n'existe pas dans le catalogue TMDB");

		const similarSource =
			(data.recommendations?.results?.length ? data.recommendations.results : data.similar?.results) || [];

		res.json({
			...card,
			playable: false,
			tagline: data.tagline || null,
			status: data.status || null,
			homepage: data.homepage || null,
			networks: (data.networks || []).map((network) => network.name),
			studios: (data.production_companies || []).slice(0, 4).map((company) => company.name),
			countries: (data.production_countries || []).map((country) => country.name),
			spokenLanguages: (data.spoken_languages || []).map((language) => language.english_name || language.name),
			budget: data.budget || null,
			revenue: data.revenue || null,
			ageRating:
				mediaType === "tv"
					? (data.content_ratings?.results || []).find((item) => item.iso_3166_1 === region)?.rating || null
					: (data.release_dates?.results || []).find((item) => item.iso_3166_1 === region)?.release_dates?.[0]
							?.certification || null,
			credits: creditsFromResponse(data.credits),
			trailer: findTrailer(data.videos?.results || []),
			providers: providersFromResponse(data["watch/providers"], region),
			legalLinks: buildLegalLinks({ title: card.title, year: card.year, mediaType, tmdbId }),
			screenshots: (data.images?.backdrops || [])
				.slice(0, 8)
				.map((image) => imageUrl(image.file_path, "w780"))
				.filter(Boolean),
			seasons:
				mediaType === "tv"
					? (data.seasons || [])
							.filter((season) => season.season_number > 0)
							.map((season) => ({
								seasonNumber: season.season_number,
								name: season.name,
								episodeCount: season.episode_count,
								airDate: season.air_date || null,
								poster: imageUrl(season.poster_path, "w342"),
								overview: season.overview || "",
							}))
					: [],
			similar: similarSource
				.slice(0, 18)
				.map((item) => cardFromTmdb(item, mediaType))
				.filter(Boolean),
			imdbId: data.imdb_id || data.external_ids?.imdb_id || null,
			legal: legalInfo(),
		});
	}),
);

api.use((req, res, next) => {
	next(new ApiError(404, `Route API inconnue : ${req.method} ${req.originalUrl}`));
});
