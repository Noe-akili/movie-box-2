import { ref, watch } from "vue";
import { applyPerformanceMode } from "../util/perf";

const STORAGE_KEY = "moviebox.settings.v1";

export const LANGUAGES = [
	{ code: "fr-FR", label: "Français" },
	{ code: "en-US", label: "English" },
	{ code: "es-ES", label: "Español" },
	{ code: "pt-BR", label: "Português" },
	{ code: "ar-SA", label: "العربية" },
];

export const REGIONS = [
	{ code: "FR", label: "France" },
	{ code: "BE", label: "Belgique" },
	{ code: "CA", label: "Canada" },
	{ code: "CH", label: "Suisse" },
	{ code: "US", label: "États-Unis" },
	{ code: "SN", label: "Sénégal" },
	{ code: "CI", label: "Côte d'Ivoire" },
	{ code: "CM", label: "Cameroun" },
	{ code: "MA", label: "Maroc" },
];

const DEFAULTS = {
	language: "fr-FR",
	region: "FR",
	developer: false,
	// "auto" détecte les téléphones anciens, "low" force le mode léger.
	performanceMode: "auto",
};

const readStored = () => {
	try {
		return { ...DEFAULTS, ...JSON.parse(window.localStorage.getItem(STORAGE_KEY) || "{}") };
	} catch {
		return { ...DEFAULTS };
	}
};

const settings = ref(readStored());

// Le profil performance est appliqué dès le chargement des réglages (et à
// chaque changement), avant même le premier rendu des pages.
applyPerformanceMode(settings.value.performanceMode);

watch(
	settings,
	(value) => {
		try {
			window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
		} catch {
			/* stockage indisponible (mode navigation privee) */
		}
		applyPerformanceMode(value.performanceMode);
	},
	{ deep: true },
);

export const useSettings = () => settings;

export const updateSettings = (patch) => {
	settings.value = { ...settings.value, ...patch };
};

export const languageLabel = (code) =>
	LANGUAGES.find((language) => language.code === code)?.label || code;
