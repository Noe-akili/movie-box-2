import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Lance le frontend Vite et le backend Node en meme temps (npm run dev:all).
 * Aucune dependance externe : les deux processus sont suivis et arretes ensemble.
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const viteBin = path.join(root, "node_modules", "vite", "bin", "vite.js");
const serverDir = path.join(root, "server");

if (!existsSync(viteBin)) {
	console.error("Dependances frontend manquantes : lance « npm install ».");
	process.exit(1);
}

if (!existsSync(path.join(serverDir, "node_modules"))) {
	console.error("Dependances backend manquantes : lance « npm --prefix server install ».");
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
	start("api  ", process.execPath, ["--watch", "src/index.js"], serverDir),
	start("front", process.execPath, [viteBin], root),
];

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

console.log("\nMovie Box : API (http://localhost:5174) + interface (http://localhost:5173)\n");
