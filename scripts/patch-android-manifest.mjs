import { readFileSync, writeFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Patches du projet Android généré par `tauri android init` :
 *
 * 1. Manifeste — `android:usesCleartextTraffic="true"` : depuis Android 9 le
 *    cleartext est bloqué par défaut, ce qui couperait les images/médias en
 *    `http://` et le serveur média loopback `http://127.0.0.1`.
 *
 * 2. build.gradle.kts — `targetSdk = 34`. À partir d'Android 15, une app qui
 *    cible l'API 35+ est affichée bord-à-bord : la barre de navigation système
 *    recouvre le bas de la WebView et la barre d'onglets de l'app devient
 *    invisible. La WebView Android ne remonte pas correctement l'encart de la
 *    barre de navigation (`env(safe-area-inset-bottom)` = 0) et l'opt-out
 *    `windowOptOutEdgeToEdgeEnforcement` est désactivé dès que targetSdk ≥ 36.
 *    Cibler l'API 34 évite l'affichage bord-à-bord et la WebView est insetée
 *    au-dessus des barres système, sans code Kotlin.
 *
 * Idempotent : relance sans risque si les patches sont déjà appliqués.
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const androidAppDir = path.join(root, "src-tauri", "gen", "android", "app");

// --- 1. Manifeste : trafic en clair -----------------------------------------
const manifest = path.join(androidAppDir, "src", "main", "AndroidManifest.xml");

if (!existsSync(manifest)) {
	console.error(`AndroidManifest.xml introuvable : ${manifest}`);
	process.exit(1);
}

let patched = readFileSync(manifest, "utf8");
const changes = [];

// Le template Tauri écrit `android:usesCleartextTraffic="${usesCleartextTraffic}"`
// (placeholder Gradle) qui vaut "false" en release et "true" en debug. L'ancien
// test `includes("usesCleartextTraffic")` voyait déjà le placeholder et
// considérait le manifest comme patché : le trafic en clair restait donc
// bloqué en release. On force ici la valeur "true" quelle que soit la forme.
const CLEARTEXT = /android:usesCleartextTraffic="[^"]*"/;
if (CLEARTEXT.test(patched) && !/android:usesCleartextTraffic="true"/.test(patched)) {
	patched = patched.replace(CLEARTEXT, 'android:usesCleartextTraffic="true"');
	changes.push('manifest: usesCleartextTraffic="true"');
}

if (!/android:usesCleartextTraffic=/.test(patched)) {
	const next = patched.replace(
		/<application\b/,
		(tag) => `${tag}\n        android:usesCleartextTraffic="true"`,
	);
	if (next === patched) {
		console.error("Balise <application> introuvable : patch impossible.");
		process.exit(1);
	}
	patched = next;
	changes.push('manifest: usesCleartextTraffic="true"');
}

// --- 1b. Caméra : scanner le QR code d'une salle de partage ------------------
// `getUserMedia` passe par `onPermissionRequest` de la WebView, qui demande
// ensuite la permission Android au runtime. Encore faut-il qu'elle soit
// déclarée dans le manifeste, sinon la demande est refusée d'office.
if (!patched.includes("android.permission.CAMERA")) {
	const next = patched.replace(
		/<application\b/,
		(tag) =>
			`    <uses-permission android:name="android.permission.CAMERA" />\n` +
			`    <uses-feature android:name="android.hardware.camera" android:required="false" />\n\n${tag}`,
	);
	if (next === patched) {
		console.error("Balise <application> introuvable : permission caméra impossible.");
		process.exit(1);
	}
	patched = next;
	changes.push("manifest: permission CAMERA");
}

if (changes.length === 0) {
	console.log("Manifest deja patche.");
} else {
	writeFileSync(manifest, patched);
	console.log(`Manifest patche : ${changes.join(", ")}.`);
}

// --- 2. build.gradle.kts : targetSdk 34 (pas d'affichage bord-à-bord) --------
const gradle = path.join(androidAppDir, "build.gradle.kts");

if (!existsSync(gradle)) {
	console.error(`build.gradle.kts introuvable : ${gradle}`);
	process.exit(1);
}

const TARGET_SDK = /(\btargetSdk\s*=\s*)(\d+)/;
const gradleContents = readFileSync(gradle, "utf8");
const match = gradleContents.match(TARGET_SDK);

if (!match) {
	console.error("Champ targetSdk introuvable dans build.gradle.kts : patch impossible.");
	process.exit(1);
}

if (match[2] === "34") {
	console.log("build.gradle.kts deja patche (targetSdk = 34).");
} else {
	writeFileSync(gradle, gradleContents.replace(TARGET_SDK, "$134"));
	console.log(`build.gradle.kts patche : targetSdk ${match[2]} -> 34.`);
}
