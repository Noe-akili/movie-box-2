import { computed, ref } from "vue";

/**
 * Profil de performance de l'appareil.
 *
 * Les WebView Android anciennes (1–2 Go de RAM, 2–4 cœurs) n'arrivent pas à
 * suivre les effets GPU (backdrop-filter, kenburns) ni les longues listes de
 * cartes : on détecte ces appareils une fois au démarrage et on expose un
 * profil « low » consommé par les composants et la feuille de style.
 *
 * Le profil peut être forcé dans les réglages (`moviebox.settings.v1`,
 * `performanceMode: "auto" | "low" | "high"`).
 */

const SETTINGS_KEY = "moviebox.settings.v1";
const FORCED_KEY = "moviebox.perf.mode";

const mode = ref("auto");
const low = ref(false);
const reducedMotion = ref(false);

const readForced = () => {
	try {
		const forced = window.localStorage.getItem(FORCED_KEY);
		if (forced === "low" || forced === "high" || forced === "auto") return forced;
	} catch {
		/* stockage indisponible */
	}
	return "auto";
};

const readSettingsMode = () => {
	try {
		const parsed = JSON.parse(window.localStorage.getItem(SETTINGS_KEY) || "{}");
		if (parsed.performanceMode === "low" || parsed.performanceMode === "high") {
			return parsed.performanceMode;
		}
	} catch {
		/* stockage indisponible */
	}
	return readForced();
};

const detectLow = () => {
	if (typeof navigator === "undefined") return false;

	const memory = Number(navigator.deviceMemory) || 0;
	const cores = Number(navigator.hardwareConcurrency) || 0;
	const connection = navigator.connection || navigator.mozConnection || navigator.webkitConnection;
	const saveData = Boolean(connection?.saveData);
	const effectiveType = String(connection?.effectiveType || "");
	const slowNetwork = effectiveType === "slow-2g" || effectiveType === "2g";
	const coarse = typeof window !== "undefined" && window.matchMedia
		? window.matchMedia("(pointer: coarse)").matches
		: false;
	const media = typeof window !== "undefined" && window.matchMedia
		? window.matchMedia("(prefers-reduced-motion: reduce)").matches
		: false;

	reducedMotion.value = media;

	// `deviceMemory` est absent sur beaucoup d'anciens WebView : les cœurs et
	// l'absence de mémoire renseignée servent alors de signal de repli. Le
	// pointeur grossier évite de classer un ordinateur 4 cœurs en « faible ».
	if (saveData || slowNetwork) return true;
	if (memory > 0 && memory <= 2) return true;
	if (coarse && memory > 0 && memory <= 4 && cores > 0 && cores <= 4) return true;
	if (coarse && cores > 0 && cores <= 4) return true;
	if (memory === 0 && cores === 0 && coarse) return true;
	return media;
};

const apply = () => {
	if (typeof document === "undefined") return;
	const root = document.documentElement;
	root.dataset.perf = low.value ? "low" : "high";
	root.dataset.reducedMotion = reducedMotion.value ? "true" : "false";
};

/** Applique un mode ("auto", "low", "high") et met à jour le DOM. */
export const applyPerformanceMode = (next = "auto") => {
	const safe = next === "low" || next === "high" ? next : "auto";
	mode.value = safe;
	low.value = safe === "low" ? true : safe === "high" ? false : detectLow();
	apply();
	return low.value;
};

/** À appeler avant le montage de l'application. */
export const initPerformance = () => applyPerformanceMode(readSettingsMode());

export const performanceMode = computed(() => mode.value);
export const isLowPerformance = computed(() => low.value);
export const prefersReducedMotion = computed(() => reducedMotion.value);

/** Nombre de lignes demandées par page sur les listes infinies. */
export const rowsPerPage = () => (low.value ? 18 : 30);

/** Marge de déclenchement du scroll infini (px autour du témoin). */
export const infiniteRootMargin = () => (low.value ? "250px" : "600px");

/** Rangées supplémentaires rendues au-dessus/en dessous de la zone visible. */
export const gridOverscan = () => (low.value ? 1 : 2);

/** Préchargement des chunks de routes à l'inactivité. */
export const canPrefetchRoutes = () => {
	if (low.value || reducedMotion.value) return false;
	const connection = typeof navigator !== "undefined"
		? navigator.connection || navigator.mozConnection || navigator.webkitConnection
		: null;
	return !connection?.saveData;
};
