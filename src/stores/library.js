import { computed, ref, watch } from "vue";

const STORAGE_KEY = "moviebox.library.v1";
const MAX_HISTORY = 40;

/** Sous-ensemble d'une carte conserve localement (favoris + historique). */
export const toLibraryItem = (item) => {
	if (!item?.id || !item?.mediaType) return null;

	return {
		id: item.id,
		mediaType: item.mediaType,
		tmdbId: item.tmdbId ?? null,
		archiveId: item.archiveId ?? null,
		title: item.title || "Sans titre",
		poster: item.poster || null,
		backdrop: item.backdrop || null,
		badge: item.badge || null,
		year: item.year ?? null,
		rating: item.rating ?? null,
		runtime: item.runtime ?? null,
		savedAt: Date.now(),
	};
};

const readStored = () => {
	try {
		const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) || "{}");
		return {
			favorites: Array.isArray(parsed.favorites) ? parsed.favorites : [],
			history: Array.isArray(parsed.history) ? parsed.history : [],
		};
	} catch {
		return { favorites: [], history: [] };
	}
};

const state = ref(readStored());

watch(
	state,
	(value) => {
		try {
			window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
		} catch {
			/* stockage indisponible */
		}
	},
	{ deep: true },
);

const favorites = computed(() => state.value.favorites);
const history = computed(() => state.value.history);

// Set recalculé uniquement quand les favoris changent : les cartes d'une
// longue liste ne relancent plus un parcours O(n) à chaque rendu.
const favoriteIds = computed(() => new Set(state.value.favorites.map((entry) => entry.id)));

const isFavorite = (item) => {
	if (!item?.id) return false;
	return favoriteIds.value.has(item.id);
};

const toggleFavorite = (item) => {
	const entry = toLibraryItem(item);
	if (!entry) return false;

	const alreadySaved = favoriteIds.value.has(entry.id);
	state.value = {
		...state.value,
		favorites: alreadySaved
			? state.value.favorites.filter((saved) => saved.id !== entry.id)
			: [entry, ...state.value.favorites],
	};

	return !alreadySaved;
};

const addToHistory = (item) => {
	const entry = toLibraryItem(item);
	if (!entry) return;

	const rest = state.value.history.filter((saved) => saved.id !== entry.id);
	state.value = { ...state.value, history: [entry, ...rest].slice(0, MAX_HISTORY) };
};

const removeFromHistory = (id) => {
	state.value = { ...state.value, history: state.value.history.filter((entry) => entry.id !== id) };
};

const clearHistory = () => {
	state.value = { ...state.value, history: [] };
};

const clearFavorites = () => {
	state.value = { ...state.value, favorites: [] };
};

export const useLibrary = () => ({
	favorites,
	history,
	isFavorite,
	toggleFavorite,
	addToHistory,
	removeFromHistory,
	clearHistory,
	clearFavorites,
});
