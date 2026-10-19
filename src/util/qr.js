/**
 * Générateur de QR code minimal, sans dépendance.
 *
 * Portée volontairement réduite : mode octet, correction d'erreur niveau « L »,
 * versions 1 à 5 (une seule séquence de blocs, jusqu'à 106 octets) — largement
 * suffisant pour une URL de partage locale. Les tables et l'algorithme suivent
 * la norme ISO/IEC 18004 (placement des motifs, Reed-Solomon, masquage).
 */

/** Caractéristiques par version : taille, octets de données, octets de correction, centre d'alignement. */
const VERSIONS = [
	null,
	{ size: 21, data: 19, ecc: 7, align: null },
	{ size: 25, data: 34, ecc: 10, align: 18 },
	{ size: 29, data: 55, ecc: 15, align: 22 },
	{ size: 33, data: 80, ecc: 20, align: 26 },
	{ size: 37, data: 108, ecc: 26, align: 30 },
];

// Corps de Galois GF(256), polynôme primitif 0x11d.
const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
(() => {
	let value = 1;
	for (let index = 0; index < 255; index += 1) {
		EXP[index] = value;
		LOG[value] = index;
		value <<= 1;
		if (value & 0x100) value ^= 0x11d;
	}
	for (let index = 255; index < 512; index += 1) EXP[index] = EXP[index - 255];
})();

const gfMul = (a, b) => (a === 0 || b === 0 ? 0 : EXP[LOG[a] + LOG[b]]);

/** Multiplication de polynômes (coefficient de tête en premier). */
const polyMul = (a, b) => {
	const out = new Array(a.length + b.length - 1).fill(0);
	for (let i = 0; i < a.length; i += 1) {
		for (let j = 0; j < b.length; j += 1) out[i + j] ^= gfMul(a[i], b[j]);
	}
	return out;
};

/** Polynôme générateur de degré `degree` : ∏ (x + α^i). */
const rsGenerator = (degree) => {
	let poly = [1];
	for (let index = 0; index < degree; index += 1) poly = polyMul(poly, [1, EXP[index]]);
	return poly;
};

/** Codewords de correction Reed-Solomon pour un unique bloc de données. */
const rsEncode = (data, eccLength) => {
	const generator = rsGenerator(eccLength);
	const remainder = new Array(eccLength).fill(0);
	for (const byte of data) {
		const factor = byte ^ remainder[0];
		remainder.shift();
		remainder.push(0);
		for (let index = 0; index < eccLength; index += 1) {
			remainder[index] ^= gfMul(generator[index + 1], factor);
		}
	}
	return remainder;
};

/** Masques de données (indice de la norme). */
const MASKS = [
	(i, j) => (i + j) % 2 === 0,
	(i) => i % 2 === 0,
	(i, j) => j % 3 === 0,
	(i, j) => (i + j) % 3 === 0,
	(i, j) => (Math.floor(i / 2) + Math.floor(j / 3)) % 2 === 0,
	(i, j) => ((i * j) % 2) + ((i * j) % 3) === 0,
	(i, j) => (((i * j) % 2) + ((i * j) % 3)) % 2 === 0,
	(i, j) => (((i * j) % 3) + ((i + j) % 2)) % 2 === 0,
];

/** Bit de poids `index` (LSB d'abord) du format d'information, avec code BCH. */
const formatBits = (data) => {
	let value = data << 10;
	while (bitLength(value) - bitLength(0x537) >= 0) {
		value ^= 0x537 << (bitLength(value) - bitLength(0x537));
	}
	return ((data << 10) | value) ^ 0x5412;
};

const bitLength = (value) => {
	let length = 0;
	while (value !== 0) {
		length += 1;
		value >>>= 1;
	}
	return length;
};

/** Coût d'un masque selon la norme (règles 1 à 4). */
const maskPenalty = (modules, size) => {
	let penalty = 0;

	// Règle 1 : séries de modules identiques.
	for (let row = 0; row < size; row += 1) {
		let run = 1;
		for (let col = 1; col < size; col += 1) {
			if (modules[row][col] === modules[row][col - 1]) run += 1;
			else {
				if (run >= 5) penalty += 3 + (run - 5);
				run = 1;
			}
		}
		if (run >= 5) penalty += 3 + (run - 5);
	}
	for (let col = 0; col < size; col += 1) {
		let run = 1;
		for (let row = 1; row < size; row += 1) {
			if (modules[row][col] === modules[row - 1][col]) run += 1;
			else {
				if (run >= 5) penalty += 3 + (run - 5);
				run = 1;
			}
		}
		if (run >= 5) penalty += 3 + (run - 5);
	}

	// Règle 2 : blocs 2x2 uniformes.
	for (let row = 0; row < size - 1; row += 1) {
		for (let col = 0; col < size - 1; col += 1) {
			const value = modules[row][col];
			if (
				value === modules[row][col + 1] &&
				value === modules[row + 1][col] &&
				value === modules[row + 1][col + 1]
			) {
				penalty += 3;
			}
		}
	}

	// Règle 3 : motif 1:1:3:1:1 (ressemblant à un repère).
	const hasPattern = (line) => {
		for (let index = 0; index + 6 < line.length; index += 1) {
			if (
				line[index] &&
				!line[index + 1] &&
				line[index + 2] &&
				line[index + 3] &&
				line[index + 4] &&
				!line[index + 5] &&
				line[index + 6]
			) {
				return true;
			}
		}
		return false;
	};
	for (let row = 0; row < size; row += 1) if (hasPattern(modules[row])) penalty += 40;
	for (let col = 0; col < size; col += 1) {
		const column = [];
		for (let row = 0; row < size; row += 1) column.push(modules[row][col]);
		if (hasPattern(column)) penalty += 40;
	}

	// Règle 4 : équilibre noir/blanc.
	let dark = 0;
	for (let row = 0; row < size; row += 1) {
		for (let col = 0; col < size; col += 1) if (modules[row][col]) dark += 1;
	}
	const ratio = Math.abs((100 * dark) / (size * size) - 50) / 5;
	penalty += ratio * 10;

	return penalty;
};

const buildMatrix = (version, info, codewords, mask) => {
	const size = info.size;
	const modules = Array.from({ length: size }, () => new Array(size).fill(null));
	const isFunction = Array.from({ length: size }, () => new Array(size).fill(false));

	const place = (row, col, dark) => {
		modules[row][col] = dark;
		isFunction[row][col] = true;
	};

	// Repères de position (coins) + séparateurs.
	const finder = (row, col) => {
		for (let r = -1; r <= 7; r += 1) {
			for (let c = -1; c <= 7; c += 1) {
				const rr = row + r;
				const cc = col + c;
				if (rr < 0 || rr >= size || cc < 0 || cc >= size) continue;
				const dark =
					(r >= 0 && r <= 6 && (c === 0 || c === 6)) ||
					(c >= 0 && c <= 6 && (r === 0 || r === 6)) ||
					(r >= 2 && r <= 4 && c >= 2 && c <= 4);
				place(rr, cc, dark);
			}
		}
	};
	finder(0, 0);
	finder(0, size - 7);
	finder(size - 7, 0);

	// Motifs de synchronisation.
	for (let index = 8; index < size - 8; index += 1) {
		place(6, index, index % 2 === 0);
		place(index, 6, index % 2 === 0);
	}

	// Motif d'alignement (versions 2+: un seul, en bas à droite).
	if (info.align !== null) {
		const center = info.align;
		for (let r = -2; r <= 2; r += 1) {
			for (let c = -2; c <= 2; c += 1) {
				place(center + r, center + c, Math.max(Math.abs(r), Math.abs(c)) !== 1);
			}
		}
	}

	// Réserve des zones d'information de format.
	for (let index = 0; index < 9; index += 1) {
		if (index !== 6) {
			if (!isFunction[8][index]) place(8, index, false);
			if (!isFunction[index][8]) place(index, 8, false);
		}
	}
	for (let index = 0; index < 8; index += 1) {
		if (!isFunction[8][size - 1 - index]) place(8, size - 1 - index, false);
		if (!isFunction[size - 1 - index][8]) place(size - 1 - index, 8, false);
	}
	// Module toujours sombre.
	place(size - 8, 8, true);

	// Placement des données en zigzag (deux colonnes à la fois, en boustrophédon).
	const totalBits = codewords.length * 8;
	const dataBit = (index) => (codewords[index >> 3] >> (7 - (index & 7))) & 1;
	let bitIndex = 0;
	let upward = true;
	for (let col = size - 1; col > 0; col -= 2) {
		if (col === 6) col -= 1;
		for (let step = 0; step < size; step += 1) {
			const row = upward ? size - 1 - step : step;
			for (let offset = 0; offset < 2; offset += 1) {
				const cc = col - offset;
				if (isFunction[row][cc]) continue;
				let dark = bitIndex < totalBits ? dataBit(bitIndex) === 1 : false;
				bitIndex += 1;
				if (MASKS[mask](row, cc)) dark = !dark;
				modules[row][cc] = dark;
			}
		}
		upward = !upward;
	}

	// Information de format (niveau L + masque).
	const bits = formatBits((0b01 << 3) | mask);
	for (let index = 0; index < 15; index += 1) {
		const dark = ((bits >> index) & 1) === 1;
		if (index < 6) modules[index][8] = dark;
		else if (index < 8) modules[index + 1][8] = dark;
		else modules[size - 15 + index][8] = dark;
	}
	for (let index = 0; index < 15; index += 1) {
		const dark = ((bits >> index) & 1) === 1;
		if (index < 8) modules[8][size - index - 1] = dark;
		else if (index < 9) modules[8][15 - index - 1 + 1] = dark;
		else modules[8][15 - index - 1] = dark;
	}
	modules[size - 8][8] = true;

	return modules;
};

/**
 * Encode `text` et renvoie `{ size, modules }` (`modules[row][col]` = booléen).
 * Renvoie `null` si le texte dépasse la capacité du générateur (106 octets).
 */
export const createQr = (text) => {
	const bytes = new TextEncoder().encode(String(text ?? ""));
	let version = 0;
	for (let candidate = 1; candidate < VERSIONS.length; candidate += 1) {
		if (4 + 8 + bytes.length * 8 <= VERSIONS[candidate].data * 8) {
			version = candidate;
			break;
		}
	}
	if (version === 0) return null;

	const info = VERSIONS[version];
	const bits = [];
	const push = (value, length) => {
		for (let index = length - 1; index >= 0; index -= 1) bits.push((value >> index) & 1);
	};

	push(0b0100, 4);
	push(bytes.length, 8);
	for (const byte of bytes) push(byte, 8);
	const capacity = info.data * 8;
	for (let index = 0; index < 4 && bits.length < capacity; index += 1) bits.push(0);
	while (bits.length % 8 !== 0) bits.push(0);

	const dataBytes = [];
	for (let index = 0; index < bits.length; index += 8) {
		let byte = 0;
		for (let offset = 0; offset < 8; offset += 1) byte = (byte << 1) | bits[index + offset];
		dataBytes.push(byte);
	}
	const pads = [0xec, 0x11];
	let pad = 0;
	while (dataBytes.length < info.data) dataBytes.push(pads[pad++ % 2]);

	const codewords = dataBytes.concat(rsEncode(dataBytes, info.ecc));

	let best = null;
	for (let mask = 0; mask < 8; mask += 1) {
		const modules = buildMatrix(version, info, codewords, mask);
		const penalty = maskPenalty(modules, info.size);
		if (best === null || penalty < best.penalty) best = { modules, penalty };
	}

	return { size: info.size, modules: best.modules };
};

export default createQr;
