import assert from "node:assert/strict";
import test from "node:test";
import { createQr } from "./qr.js";

const finderOk = (modules, row, col) => {
	for (let r = 0; r < 7; r += 1) {
		for (let c = 0; c < 7; c += 1) {
			const dark =
				r === 0 || r === 6 || c === 0 || c === 6 || (r >= 2 && r <= 4 && c >= 2 && c <= 4);
			if (Boolean(modules[row + r][col + c]) !== dark) return false;
		}
	}
	return true;
};

test("taille selon la longueur (version 1 pour un texte court)", () => {
	const qr = createQr("HELLO");
	assert.equal(qr.size, 21);
});

test("repères de position aux trois coins", () => {
	const qr = createQr("http://192.168.1.10:8090/abc123/");
	assert.equal(qr.size, qr.modules.length);
	assert.ok(finderOk(qr.modules, 0, 0), "coin haut-gauche");
	assert.ok(finderOk(qr.modules, 0, qr.size - 7), "coin haut-droit");
	assert.ok(finderOk(qr.modules, qr.size - 7, 0), "coin bas-gauche");
});

test("génération déterministe", () => {
	const value = "http://10.0.0.7:8090/deadbeefcafebabe/";
	assert.deepEqual(createQr(value), createQr(value));
});

test("refuse un texte au-delà de la capacité", () => {
	assert.equal(createQr("x".repeat(200)), null);
});
