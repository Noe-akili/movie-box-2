import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Lance le frontend Vite et le backend Rust en meme temps (npm run dev:all).
 * Aucune dependance externe : les deux processus sont suivis et arretes ensemble.
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const viteBin = path.join(root, "node_modules", "vite", "bin", "vite.js");
const cargoManifest = path.join(root, "server-rust", "Cargo.toml");

// Source vidéo principale : service MovieBox local, vendu à la racine du projet
// (source_server.py + paquet moviebox_api + environnement .venv312).
const sourceDir = process.env.MOVIEBOX_SOURCE_DIR || root;
const sourcePython =
	process.env.MOVIEBOX_SOURCE_PYTHON || path.join(sourceDir, ".venv312", "bin", "python");
const sourceScript = path.join(sourceDir, "source_server.py");
const sourceUrl = process.env.INTERNAL_API_URL || "http://127.0.0.1:8000";
process.env.INTERNAL_API_URL = sourceUrl;

if (!existsSync(viteBin)) {
	console.error("Dependances frontend manquantes : lance « npm install ».");
	process.exit(1);
}

if (!existsSync(cargoManifest)) {
	console.error("Backend Rust introuvable : dossier « server-rust » manquant.");
	process.exit(1);
}

const prefix = (name, stream, target) => {
	let buffer = "";

	stream.on("data", (chunk) => {
		buffer += chunk.toString();
		const lines = buffer.split("\n");
		buffer = lines.pop() ?? "";

		for (const line of lines) {
			if (line.trim()) target.write(`[${name}] ${line}\n`);
		}
	});
};

const start = (name, command, args, cwd) => {
	const child = spawn(command, args, { cwd, env: process.env });

	prefix(name, child.stdout, process.stdout);
	prefix(name, child.stderr, process.stderr);
	child.on("exit", (code) => {
		if (code !== 0 && code !== null) console.log(`[${name}] arret (code ${code})`);
	});

	return child;
};

const children = [
	start("api  ", "cargo", ["run", "--manifest-path", cargoManifest], root),
	start("front", process.execPath, [viteBin], root),
];

if (existsSync(sourceScript) && existsSync(sourcePython)) {
	children.unshift(start("src  ", sourcePython, [sourceScript], sourceDir));
} else {
	console.warn(
		`[src  ] source MovieBox introuvable (${sourceScript}). ` +
			"Crée l'environnement : « uv venv .venv312 && uv pip install -r requirements.txt » " +
			"(ou définis MOVIEBOX_SOURCE_DIR / MOVIEBOX_SOURCE_PYTHON).",
	);
}

const shutdown = () => {
	for (const child of children) {
		if (!child.killed) child.kill("SIGTERM");
	}
};

process.on("SIGINT", () => {
	shutdown();
	process.exit(0);
});

process.on("SIGTERM", () => {
	shutdown();
	process.exit(0);
});

process.on("exit", shutdown);

console.log(
	`\nMovie Box : source ${sourceUrl} + API (http://localhost:5174) + interface (http://localhost:5173)\n`,
);
