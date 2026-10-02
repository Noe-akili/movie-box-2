import { ref, watch } from "vue";

const STORAGE_KEY = "moviebox.searches.v1";
const MAX_ITEMS = 10;

const readStored = () => {
	try {
		const parsed = JSON.parse(window.localStorage.getItem(STORAGE_KEY) || "[]");
		return Array.isArray(parsed) ? parsed.filter((item) => typeof item === "string") : [];
	} catch {
		return [];
	}
};

const recent = ref(readStored());

watch(
	recent,
	(value) => {
		try {
			window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
		} catch {
			/* stockage indisponible */
		}
	},
	{ deep: true },
);

export const useRecentSearches = () => {
	const push = (query) => {
		const value = String(query || "").trim();
		if (value.length < 2) return;

		recent.value = [value, ...recent.value.filter((item) => item.toLowerCase() !== value.toLowerCase())].slice(
			0,
			MAX_ITEMS,
		);
	};

	const remove = (query) => {
		recent.value = recent.value.filter((item) => item !== query);
	};

	const clear = () => {
		recent.value = [];
	};

	return { recent, push, remove, clear };
};
