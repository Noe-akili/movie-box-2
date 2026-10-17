import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import "./index.css";

/**
 * Démarrage de l'application.
 *
 * Sur Android (Tauri), le backend Rust est embarqué dans le processus de l'app.
 * Les appels API passent par l'IPC (aucun serveur réseau) et les flux média par
 * le protocole `moviebox://`. On interroge Tauri pour connaître cette origine
 * virtuelle avant de monter l'app, afin que les balises `<video>` visent le bon
 * protocole.
 */
async function waitForTauri(timeoutMs = 3000) {
	const start = Date.now();
	while (Date.now() - start < timeoutMs) {
		if (
			window.__TAURI__?.core?.invoke ||
			window.__TAURI__?.invoke ||
			window.__TAURI_INTERNALS__?.invoke
		) {
			return window.__TAURI__ ?? {};
		}
		await new Promise((r) => setTimeout(r, 50));
	}
	return window.__TAURI__ ?? null;
}

async function bootstrap() {
	const tauri = await waitForTauri();
	const invoke =
		tauri?.core?.invoke ||
		tauri?.invoke ||
		window.__TAURI_INTERNALS__?.invoke;

	if (invoke) {
		try {
			const origin = await invoke("server_origin");
			if (origin) {
				window.__SERVER_ORIGIN__ = origin;
				window.__API_BASE_URL__ = `${origin}/api`;
			}
		} catch (error) {
			console.error("[movie-box] origine du serveur embarqué indisponible", error);
		}
	}

	createApp(App).use(router).mount("#root");
	prefetchCommonRoutes();
}

/**
 * Préchauffe les écrans les plus probables une fois la page interactive.
 * Les chunks arrivent en tâche de fond : la première navigation vers une fiche
 * ou la lecture est quasi instantanée, sans retarder le premier rendu.
 */
function prefetchCommonRoutes() {
	if (typeof window === "undefined") return;
	const load = () =>
		Promise.allSettled([
			import("./pages/FreePage.vue"),
			import("./pages/DetailPage.vue"),
			import("./pages/WatchPage.vue"),
			import("./pages/BrowsePage.vue"),
		]);
	if ("requestIdleCallback" in window) {
		window.requestIdleCallback(load, { timeout: 2500 });
	} else {
		window.setTimeout(load, 1500);
	}
}

bootstrap();
