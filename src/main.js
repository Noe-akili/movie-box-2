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
async function bootstrap() {
	const tauri = window.__TAURI__;

	if (tauri?.core?.invoke) {
		try {
			const origin = await tauri.core.invoke("server_origin");
			if (origin) {
				window.__SERVER_ORIGIN__ = origin;
				window.__API_BASE_URL__ = `${origin}/api`;
			}
		} catch (error) {
			console.error("[movie-box] origine du serveur embarqué indisponible", error);
		}
	}

	createApp(App).use(router).mount("#root");
}

bootstrap();
