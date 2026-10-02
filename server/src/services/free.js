import { config } from "../config.js";
import {
	ARCHIVE_COLLECTIONS,
	ARCHIVE_LICENSES,
	DEFAULT_ARCHIVE_COLLECTION,
	getArchiveFile,
	getArchiveTitle,
	searchArchive,
} from "./archive.js";
import {
	COMMONS_CATEGORIES,
	DEFAULT_COMMONS_CATEGORY,
	commonsRemoteUrl,
	getCommonsFile,
	getCommonsTitle,
	isCommonsId,
	searchCommons,
} from "./commons.js";

/**
 * Couche « sources gratuites » : une seule porte d'entree pour les catalogues libres de droits.
 * Internet Archive et Wikimedia Commons exposent le meme contrat de donnees (carte, flux, licence),
 * si bien que les fiches, le lecteur et les telechargements fonctionnent sans connaitre le fournisseur.
 * Le fournisseur est porte par l'identifiant : `commons:File:...` ou un identifiant Internet Archive.
 */

export const FREE_SOURCES = [
	{ id: "archive", label: "Internet Archive", note: "Domaine public & Creative Commons" },
	{ id: "commons", label: "Wikimedia Commons", note: "Licences vérifiées par la communauté" },
];

export const DEFAULT_FREE_SOURCE = "archive";

const DEFAULT_SOURCE = FREE_SOURCES[0];

export const resolveFreeSource = (value) => FREE_SOURCES.find((source) => source.id === value) || DEFAULT_SOURCE;

export const isCommonsSource = (value) => resolveFreeSource(value).id === "commons";

/** Collections / categories proposees pour une source. */
export const freeCollections = (source) =>
	isCommonsSource(source)
		? COMMONS_CATEGORIES.map((category) => ({ ...category }))
		: ARCHIVE_COLLECTIONS.map((collection) => ({ ...collection }));

export const defaultFreeCollection = (source) =>
	isCommonsSource(source) ? DEFAULT_COMMONS_CATEGORY : DEFAULT_ARCHIVE_COLLECTION;

/**
 * Filtres de droits proposes pour une source.
 * Commons n'accepte que des œuvres libres : un seul filtre, mais il est verifie fichier par fichier.
 */
export const freeLicenses = (source) =>
	isCommonsSource(source)
		? [{ id: "free", label: "Domaine public & Creative Commons (vérifié par Commons)" }]
		: ARCHIVE_LICENSES.map(({ id, label }) => ({ id, label }));

export const searchFree = ({ source, ...params } = {}) =>
	isCommonsSource(source)
		? searchCommons({
				q: params.q,
				page: params.page,
				rows: params.rows,
				// `incategory` ne matche que l'appartenance exacte : une recherche libre doit l'ignorer.
				category: params.q ? "" : params.collection,
			})
		: searchArchive(params);

/** Fiche complete, quel que soit le fournisseur indique par l'identifiant. */
export const getFreeTitle = (identifier) => (isCommonsId(identifier) ? getCommonsTitle(identifier) : getArchiveTitle(identifier));

/** Validation d'un fichier a telecharger, quel que soit le fournisseur. */
export const getFreeFile = (sourceId, fileName) =>
	isCommonsId(sourceId) ? getCommonsFile(sourceId, fileName) : getArchiveFile(sourceId, fileName);

/**
 * URL distante d'un fichier deja valide (le telechargement repart de la ou il s'est arrete).
 * `null` si le fournisseur ou le nom de fichier n'est pas reconnu.
 */
export const freeRemoteUrl = (sourceId, fileName) =>
	isCommonsId(sourceId)
		? commonsRemoteUrl(sourceId, fileName)
		: `${config.archiveBaseUrl}/download/${encodeURIComponent(sourceId)}/${encodeURIComponent(fileName)}`;

/** Libelle du fournisseur d'un identifiant, utilise dans les messages d'erreur et l'interface. */
export const freeSourceLabel = (sourceId) => (isCommonsId(sourceId) ? "Wikimedia Commons" : "Internet Archive");
