import { computed, ref, watch } from "vue";
import {
	cancelDownload as apiCancel,
	createDownload as apiCreate,
	deleteDownload as apiDelete,
	fetchDownloads,
	retryDownload as apiRetry,
} from "../api/client";
import { syncBackgroundDownloads } from "../util/background";

/**
 * Etat des telechargements cote interface.
 * Le backend fait le travail (stockage + progression) : ici on lit son etat en boucle
 * (plus vite pendant un transfert) et on expose des aides pour lancer ou supprimer.
 */
const items = ref([]);
const summary = ref({
	total: 0,
	completed: 0,
	active: 0,
	queued: 0,
	errors: 0,
	paused: 0,
	completedBytes: 0,
	activeBytes: 0,
	maxBytes: 0,
	freeBytes: null,
});
const isLoading = ref(false);
const lastError = ref(null);

const ACTIVE_STATUSES = ["queued", "downloading"];

let timer = null;
let started = false;

const hasActive = computed(() => items.value.some((item) => ACTIVE_STATUSES.includes(item.status)));
const activeCount = computed(() => items.value.filter((item) => ACTIVE_STATUSES.includes(item.status)).length);
const completedItems = computed(() => items.value.filter((item) => item.status === "completed"));
const pendingItems = computed(() => items.value.filter((item) => item.status !== "completed"));

// Android : active le service de premier plan tant qu'un transfert est en
// cours, pour que la mise en arrière-plan ne gèle pas les téléchargements.
watch(hasActive, (active) => {
	syncBackgroundDownloads(active);
});

const refresh = async ({ silent = false } = {}) => {
	if (!silent) isLoading.value = true;

	try {
		const data = await fetchDownloads();
		items.value = data.items || [];
		summary.value = { ...summary.value, ...(data.summary || {}) };
		lastError.value = null;
	} catch (error) {
		lastError.value = error.message;
	} finally {
		isLoading.value = false;
	}
};

/**
 * Boucle adaptative : 1,2 s pendant un transfert, 30 s au repos. Aucune requête
 * quand l'app est en arrière-plan : sur un téléphone ancien, le polling inutile
 * réveille le CPU et vide la batterie.
 */
const schedule = () => {
	window.clearTimeout(timer);
	if (typeof document !== "undefined" && document.hidden) {
		timer = null;
		return;
	}
	timer = window.setTimeout(async () => {
		if (!document.hidden) await refresh({ silent: true });
		schedule();
	}, hasActive.value ? 1200 : 30000);
};

let visibilityBound = false;
const bindVisibility = () => {
	if (visibilityBound || typeof document === "undefined") return;
	visibilityBound = true;
	document.addEventListener("visibilitychange", () => {
		if (document.hidden) {
			window.clearTimeout(timer);
			timer = null;
		} else {
			refresh({ silent: true }).finally(schedule);
		}
	});
};

const upsert = (entry) => {
	const index = items.value.findIndex((item) => item.id === entry.id);
	if (index === -1) items.value = [entry, ...items.value];
	else items.value = items.value.map((item) => (item.id === entry.id ? entry : item));
};

/** Retrouve le telechargement d'un fichier donne (identifiant d'archive + nom de fichier). */
const entryFor = (sourceId, fileName) =>
	items.value.find((item) => item.sourceId === sourceId && item.fileName === fileName) || null;

const start = async (payload) => {
	const entry = await apiCreate(payload);
	upsert(entry);
	schedule();
	return entry;
};

const pause = async (id) => {
	const entry = await apiCancel(id);
	upsert(entry);
	return entry;
};

const retry = async (id) => {
	const entry = await apiRetry(id);
	upsert(entry);
	schedule();
	return entry;
};

const remove = async (id) => {
	await apiDelete(id);
	items.value = items.value.filter((item) => item.id !== id);
	await refresh({ silent: true });
};

/** Demarre la lecture de l'etat (appelee une seule fois par App.vue). */
export const ensureDownloadsPolling = () => {
	if (started) return;
	started = true;
	bindVisibility();

	// Premier appel différé : le démarrage de l'app reste réactif sur mobile.
	const kickoff = () => refresh({ silent: true }).finally(schedule);
	if (typeof window !== "undefined" && "requestIdleCallback" in window) {
		window.requestIdleCallback(kickoff, { timeout: 4000 });
	} else {
		window.setTimeout(kickoff, 1500);
	}
};

export const useDownloads = () => ({
	items,
	summary,
	isLoading,
	lastError,
	hasActive,
	activeCount,
	completedItems,
	pendingItems,
	refresh,
	entryFor,
	start,
	pause,
	retry,
	remove,
});
