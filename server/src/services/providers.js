import { config } from "../config.js";

const enc = encodeURIComponent;

/**
 * Liens vers des plateformes LEGALES (y compris les offres gratuites avec publicite).
 * Aucun scraping ni flux pirate : Movie Box redirige vers l'ayant droit.
 */
export const buildLegalLinks = ({ title = "", year = null, mediaType = "movie", tmdbId = null } = {}) => {
	if (!title) return [];

	const query = [title, year].filter(Boolean).join(" ");
	const isTv = mediaType === "tv";

	return [
		{
			id: "justwatch",
			name: "JustWatch",
			description: "Comparateur officiel : trouve l'offre gratuite ou incluse pour ce titre",
			url: `https://www.justwatch.com/fr/recherche?q=${enc(title)}`,
			free: false,
			icon: "search",
		},
		{
			id: "pluto",
			name: "Pluto TV",
			description: "Films et séries gratuits financer par la publicite",
			url: `https://pluto.tv/fr/search?q=${enc(title)}`,
			free: true,
			icon: "tv",
		},
		{
			id: "tubi",
			name: "Tubi",
			description: "Catalogue gratuit avec publicite",
			url: `https://tubitv.com/search/${enc(query)}`,
			free: true,
			icon: "tv",
		},
		{
			id: "plex",
			name: "Plex",
			description: "Films gratuits avec publicite",
			url: `https://watch.plex.tv/search?query=${enc(title)}`,
			free: true,
			icon: "tv",
		},
		{
			id: "arte",
			name: "ARTE",
			description: "Films et documentaires gratuits (service public europeen)",
			url: `https://www.arte.tv/fr/search/?q=${enc(title)}`,
			free: true,
			icon: "tv",
		},
		{
			id: "youtube",
			name: "Bande-annonce sur YouTube",
			description: "Extraits et bandes-annonces officielles",
			url: `https://www.youtube.com/results?search_query=${enc(`${query} ${isTv ? "série" : "film"} bande annonce officielle`)}`,
			free: true,
			icon: "play",
		},
		{
			id: "archive",
			name: "Internet Archive",
			description: "Films du domaine public lisibles directement dans Movie Box",
			url: `/free?q=${enc(title)}`,
			free: true,
			icon: "archive",
			internal: true,
		},
		{
			id: "tmdb",
			name: "Fiche TMDB",
			description: "Source de reference des metadonnees du film",
			url: tmdbId ? `https://www.themoviedb.org/${isTv ? "tv" : "movie"}/${tmdbId}` : `https://www.themoviedb.org/search?query=${enc(title)}`,
			free: true,
			icon: "info",
			externalNote: true,
		},
	];
};

export const legalInfo = () => ({
	region: config.watchRegion,
	statement:
		"Movie Box indexe l'ensemble du catalogue cinematographique mondial et lit les films libres de droits (Internet Archive). " +
		"Pour les films sous copyright, l'application renvoie vers les plateformes officielles, dont les offres gratuites avec publicite.",
	archiveFilter:
		"Les films et series gratuits sont filtres sur les droits declares a la source : domaine public " +
		"(Creative Commons mark/zero) ou licence Creative Commons. Le catalogue gratuit est browse par collection " +
		"et par filtre de droits, jamais par recherche de liens externes.",
});
