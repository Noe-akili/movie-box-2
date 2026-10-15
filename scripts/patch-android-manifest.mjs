import { readFileSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Autorise le trafic en clair (cleartext) dans l'APK Android.
 *
 * Le backend Rust embarque ecoute sur `http://127.0.0.1:5174` ; depuis Android 9
 * (API 28) le cleartext est bloque par defaut, ce qui empecherait la WebView et
 * le lecteur video de joindre le serveur local. On ajoute donc
 * `android:usesCleartextTraffic="true"` sur la balise <application>.
 *
 * Idempotent : relance sans risque si l'attribut est deja present.
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifest = path.join(
	root,
	"src-tauri",
	"gen",
	"android",
	"app",
	"src",
	"main",
	"AndroidManifest.xml",
);

if (!existsSync(manifest)) {
	console.error(`AndroidManifest.xml introuvable : ${manifest}`);
	process.exit(1);
}

const contents = readFileSync(manifest, "utf8");

if (contents.includes("usesCleartextTraffic")) {
	console.log("Manifest deja patche (usesCleartextTraffic present).");
	process.exit(0);
}

const patched = contents.replace(/<application\b/, (tag) =>
	`${tag}\n        android:usesCleartextTraffic="true"`,
);

if (patched === contents) {
	console.error("Balise <application> introuvable : patch impossible.");
	process.exit(1);
}

writeFileSync(manifest, patched);
console.log("Manifest patche : android:usesCleartextTraffic=\"true\" ajoute.");
