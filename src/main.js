import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import { canPrefetchRoutes, initPerformance } from "./util/perf";
import "./index.css";

/**
 * Démarrage de l'application.
 *
 * Sur Android (Tauri), le backend Rust est embarqué dans le processus de l'app.
 * Les appels API passent par l'IPC (aucun serveur réseau). Les flux média sont
 * servis par un petit serveur HTTP loopback (`http://127.0.0.1:<port>`) qui gère
 * les requêtes `Range` (seek) ; sur bureau, c'est le protocole `moviebox://`.
 * On interroge Tauri pour connaître cette origine avant de monter l'app, afin
 * que les balises `<video>` visent la bonne.
 */
/**
 * Sur le web, l'IPC Tauri n'arrivera jamais : attendre 3 s avant de monter
 * l'application ne servait qu'à afficher l'écran d'ouverture plus longtemps.
 * On réserve l'attente longue aux WebView Tauri (dont l'injection peut être
 * différée sur Android).
 */
function looksLikeTauriRuntime() {
	if (typeof window === "undefined" || typeof location === "undefined") return false;
	return (
		location.protocol === "tauri:" ||
		/tauri/i.test(location.hostname || "") ||
		/tauri/i.test(navigator.userAgent || "")
	);
}

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

/**
 * Lit la valeur effective d'un encart sûr (`env(safe-area-inset-*)`) via un
 * élément sonde : `getComputedStyle` résout `env()` en pixels, ce qui permet de
 * détecter les WebView qui ne l'exposent pas.
 */
function readEnvInset(side) {
	if (typeof document === "undefined") return 0;
	const probe = document.createElement("div");
	probe.style.cssText = `position:fixed;top:0;left:0;width:0;visibility:hidden;pointer-events:none;padding-${side}:env(safe-area-inset-${side},0px)`;
	document.body.appendChild(probe);
	const property = side === "top" ? "paddingTop" : "paddingBottom";
	const value = parseFloat(getComputedStyle(probe)[property]) || 0;
	probe.remove();
	return value;
}

/**
 * Applique les encarts sûrs à la racine CSS (`--safe-top` / `--safe-bottom`).
 *
 * `env()` couvre iOS et les WebView Android récentes. Sur Android, l'APK cible
 * l'API 34 (voir `scripts/patch-android-manifest.mjs`) : l'affichage bord-à-bord
 * n'est pas imposé et la WebView est déjà insetée au-dessus des barres système,
 * donc aucun repli n'est nécessaire (les encarts `env()` valent 0).
 */
function applySafeAreaInsets() {
	if (typeof document === "undefined") return;
	const root = document.documentElement;

	const top = readEnvInset("top");
	const bottom = readEnvInset("bottom");
	if (top > 0) root.style.setProperty("--safe-top", `${top}px`);
	if (bottom > 0) root.style.setProperty("--safe-bottom", `${bottom}px`);
}

async function bootstrap() {
	// Profil appareil avant tout rendu : la feuille de style et les composants
	// savent immédiatement s'ils doivent couper les effets coûteux.
	initPerformance();

	// Injection déjà présente (Tauri bureau, `withGlobalTauri`) : aucun délai.
	// Web : aucun délai non plus. Android différé : on laisse sa chance à l'IPC.
	const injected =
		window.__TAURI__?.core?.invoke ||
		window.__TAURI__?.invoke ||
		window.__TAURI_INTERNALS__?.invoke;
	const tauri = injected
		? window.__TAURI__ ?? {}
		: looksLikeTauriRuntime()
			? await waitForTauri(3000)
			: null;
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

	applySafeAreaInsets();

	createApp(App).use(router).mount("#root");
	window.__hideBootSplash?.();
	prefetchCommonRoutes();
}

/**
 * Préchauffe les écrans les plus probables une fois la page interactive.
 * Les chunks arrivent en tâche de fond : la première navigation vers une fiche
 * ou la lecture est quasi instantanée, sans retarder le premier rendu.
 */
function prefetchCommonRoutes() {
	if (typeof window === "undefined" || !canPrefetchRoutes()) return;
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
