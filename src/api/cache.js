/**
 * Petit cache mémoire des pages de catalogue.
 *
 * Les listes infinies reviennent souvent sur les mêmes filtres (source,
 * collection, tri, page) : garder les réponses quelques minutes évite de
 * relancer le backend et de reconstruire les cartes pendant un aller-retour.
 * Le cache vit uniquement en mémoire (aucun stockage disque, aucune donnée
 * sensible) et reste volontairement petit pour les téléphones à faible RAM.
 */

const MAX_ENTRIES = 48;
const entries = new Map();

export const cacheGet = (key) => {
	const entry = entries.get(key);
	if (!entry) return null;
	if (entry.expiresAt <= Date.now()) {
		entries.delete(key);
		return null;
	}
	// Remet l'entrée en tête (LRU simple).
	entries.delete(key);
	entries.set(key, entry);
	return entry.value;
};

export const cacheSet = (key, value, ttlMs = 120_000) => {
	if (!value) return value;
	entries.delete(key);
	entries.set(key, { value, expiresAt: Date.now() + ttlMs });

	if (entries.size > MAX_ENTRIES) {
		const oldest = entries.keys().next().value;
		if (oldest !== undefined) entries.delete(oldest);
	}
	return value;
};

export const cacheDeletePrefix = (prefix) => {
	for (const key of [...entries.keys()]) {
		if (key.startsWith(prefix)) entries.delete(key);
	}
};

export const cacheClear = () => entries.clear();
