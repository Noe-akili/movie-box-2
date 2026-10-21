/**
 * Service de premier plan Android : garde le processus éveillé pendant les
 * téléchargements, même quand l'application passe en arrière-plan.
 *
 * Le travail réel est fait par le backend Rust ; ce service ne fait que
 * maintenir la durée de vie du processus. Toutes les erreurs sont silencieuses
 * (web, bureau, plugin indisponible) : ce confort ne doit jamais bloquer l'UI.
 */
let started = false;
let pending = null;

const inTauri = () =>
	typeof window !== "undefined" &&
	Boolean(window.__TAURI_INTERNALS__ || window.__TAURI__);

export const syncBackgroundDownloads = (active) => {
	if (!inTauri()) return Promise.resolve();

	if (!active) {
		if (!started) return Promise.resolve();
		started = false;
		return import("tauri-plugin-background-service")
			.then(({ stopService }) => stopService())
			.catch(() => {});
	}

	if (started) return Promise.resolve();
	if (pending) return pending;

	pending = import("tauri-plugin-background-service")
		.then(({ startService }) =>
			startService({
				serviceLabel: "Téléchargements moviebox-2",
				foregroundServiceType: "dataSync",
			}),
		)
		.then(() => {
			started = true;
		})
		.catch((error) => {
			console.warn("[moviebox-2] service d'arrière-plan indisponible", error);
		})
		.finally(() => {
			pending = null;
		});

	return pending;
};
