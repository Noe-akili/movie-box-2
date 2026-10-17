import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [vue()],
	// Chemins d'assets relatifs : indispensables pour que la WebView Android
	// (protocole embarqué `http://tauri.localhost`) résolve `./assets/*` quel
	// que soit le sous-chemin de chargement.
	base: "./",
	server: {
		host: true,
		port: 5173,
		// Toutes les requetes /api sont proxifiees vers le backend Rust (dossier server-rust/).
		proxy: {
			"/api": {
				target: process.env.VITE_PROXY_TARGET || "http://localhost:5174",
				changeOrigin: true,
			},
		},
	},
	build: {
		outDir: "dist",
		chunkSizeWarningLimit: 900,
		// Navigateurs modernes ciblés : code plus léger, pas de polyfill inutile.
		target: "es2020",
		// Le polyfill modulepreload est inutile pour nos cibles : on l'écarte.
		modulePreload: { polyfill: false },
		rollupOptions: {
			output: {
				// Sépare le socle (Vue, routeur, axios) des pages : le navigateur peut
				// les télécharger en parallèle et les garder en cache entre les versions.
				manualChunks(id) {
					if (!id.includes("node_modules")) return;
					if (id.includes("vue-router")) return "vendor-router";
					if (id.includes("/@vue/") || id.includes("/vue/")) return "vendor-vue";
					if (id.includes("axios")) return "vendor-axios";
					return "vendor";
				},
			},
		},
	},
});
