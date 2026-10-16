import { cpSync, existsSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Installe les icônes Android générées par `tauri icon` dans le projet Android.
 *
 * `npx tauri android init` ne recopie PAS `src-tauri/icons/android/*` : le projet
 * généré garde donc les icônes par défaut du CLI Tauri (l'icône "jaune/cyan"),
 * jamais celle choisie dans `src/images/logo.png`. Ce script écrase les
 * `mipmap-*` par ceux de `src-tauri/icons/android/`.
 *
 * Idempotent, et protégé contre un doublon de ressource `ic_launcher_background`
 * (défini ailleurs par le template).
 *
 * Usage : node scripts/install-android-icons.mjs
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.join(root, "src-tauri", "icons", "android");
const resDir = path.join(root, "src-tauri", "gen", "android", "app", "src", "main", "res");

function fail(message) {
	console.error(message);
	process.exit(1);
}

if (!existsSync(source)) {
	fail(`Icônes Android introuvables : ${source} (lance « npx tauri icon src/images/logo.png »).`);
}
if (!existsSync(resDir)) {
	fail(`Projet Android introuvable : ${resDir} (lance « npx tauri android init --ci »).`);
}

// 1. Remplace tous les mipmap-* (ic_launcher, ic_launcher_round, ic_launcher_foreground,
//    mipmap-anydpi-v26/ic_launcher.xml) par ceux générés depuis le logo.
let installed = 0;
for (const entry of readdirSync(source)) {
	if (!entry.startsWith("mipmap-")) continue;
	const from = path.join(source, entry);
	if (!statSync(from).isDirectory()) continue;
	const to = path.join(resDir, entry);
	rmSync(to, { recursive: true, force: true });
	cpSync(from, to, { recursive: true });
	installed++;
}
if (installed === 0) fail(`Aucun dossier mipmap-* dans ${source}.`);

// 2. Fond de l'icône adaptative : uniquement s'il n'est pas déjà défini
//    (sinon AAPT2 échoue sur une ressource dupliquée).
const backgroundSource = path.join(source, "values", "ic_launcher_background.xml");
if (existsSync(backgroundSource)) {
	const duplicate = readdirSync(resDir)
		.filter((dir) => dir.startsWith("values"))
		.map((dir) => path.join(resDir, dir))
		.filter((dir) => statSync(dir).isDirectory())
		.flatMap((dir) => readdirSync(dir).map((file) => path.join(dir, file)))
		.filter((file) => file !== path.join(resDir, "values", "ic_launcher_background.xml"))
		.filter((file) => file.endsWith(".xml"))
		.some((file) => {
			try {
				return readFileSync(file, "utf8").includes('name="ic_launcher_background"');
			} catch {
				return false;
			}
		});

	if (duplicate) {
		console.log("Fond d'icône deja defini ailleurs : on garde celui du template.");
	} else {
		cpSync(backgroundSource, path.join(resDir, "values", "ic_launcher_background.xml"));
		console.log("Fond d'icône adaptative installe.");
	}
}

console.log(`Icônes Android installees depuis ${path.relative(root, source)} (${installed} dossiers mipmap).`);
