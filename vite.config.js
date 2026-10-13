import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [vue()],
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
	},
});
