import { computed, ref } from "vue";
import {
	clearTransferFiles,
	fetchTransfer,
	removeTransferFile,
	startTransfer as apiStart,
	stopTransfer as apiStop,
	updateTransferFiles,
} from "../api/client";

/**
 * Etat du partage Wi-Fi local cote interface.
 * Le backend heberge la "salle" ; ici on lit son etat en boucle quand l'ecran
 * Transferts est ouvert et on expose les actions (demarrer, arreter, selectionner).
 */
const state = ref({
	running: false,
	url: null,
	port: null,
	shared: [],
	received: [],
	files: [],
});
const isLoading = ref(false);
const lastError = ref(null);

let timer = null;
let polling = false;

const running = computed(() => Boolean(state.value.running));
const url = computed(() => state.value.url || "");
const received = computed(() => state.value.received || []);
const files = computed(() => state.value.files || []);
const shared = computed(() => state.value.shared || []);

const apply = (patch) => {
	state.value = { ...state.value, ...(patch || {}) };
};

const refresh = async ({ silent = false } = {}) => {
	if (!silent) isLoading.value = true;
	try {
		apply(await fetchTransfer());
		lastError.value = null;
	} catch (error) {
		lastError.value = error.message;
	} finally {
		isLoading.value = false;
	}
};

const schedule = () => {
	window.clearTimeout(timer);
	if (!polling) return;
	timer = window.setTimeout(async () => {
		await refresh({ silent: true });
		schedule();
	}, running.value ? 3000 : 6000);
};

/** Demarre la lecture de l'etat (appele au montage de l'ecran Transferts). */
export const startTransferPolling = () => {
	if (polling) return;
	polling = true;
	refresh({ silent: true }).finally(schedule);
};

/** Arrete la lecture de l'etat (au demontage de l'ecran Transferts). */
export const stopTransferPolling = () => {
	polling = false;
	window.clearTimeout(timer);
	timer = null;
};

const start = async (ids) => {
	apply(await apiStart(Array.isArray(ids) ? { files: ids } : {}));
	await refresh({ silent: true });
};

const stop = async () => {
	apply(await apiStop());
	await refresh({ silent: true });
};

const setShared = async (ids) => {
	apply(await updateTransferFiles(ids));
};

const removeReceived = async (id) => {
	await removeTransferFile(id);
	state.value = { ...state.value, received: received.value.filter((file) => file.id !== id) };
};

const clearReceived = async () => {
	await clearTransferFiles();
	state.value = { ...state.value, received: [] };
};

export const useTransfer = () => ({
	state,
	isLoading,
	lastError,
	running,
	url,
	received,
	files,
	shared,
	refresh,
	start,
	stop,
	setShared,
	removeReceived,
	clearReceived,
});
