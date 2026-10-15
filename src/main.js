import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import "./index.css";

/**
 * Démarrage de l'application.
 *
 * Sur Android (Tauri), le backend Rust est embarqué dans le processus de l'app
 * et écoute sur `127.0.0.1:<port>`. On interroge Tauri pour connaître cette
 * origine avant de monter l'app, afin que le client HTTP (`src/api/client.js`)
 * et les balises `<video>` visent le bon serveur.
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
