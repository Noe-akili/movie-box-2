import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";

/**
 * Recadre le premier plan de l'icône adaptative Android.
 *
 * `npx tauri icon` génère un `ic_launcher_foreground.png` qui remplit tout le
 * canevas (108 dp). Or Android n'affiche que la zone sûre centrale (~66 %) :
 * le reste est rogné par le masque du lanceur, ce qui donne une icône « zoomée ».
 *
 * Ce script réduit le contenu visible et le recentre sur un canevas transparent,
 * pour qu'il tienne dans la zone sûre. Il est idempotent : relancer le script
 * (ou partir d'un logo déjà margé) conserve le même cadrage.
 *
 * Aucune dépendance : lecture/écriture PNG RGBA 8 bits via `node:zlib`.
 *
 * Usage : node scripts/pad-android-foreground.mjs [dossier-icones]
 */
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const iconsRoot = path.resolve(root, process.argv[2] || "src-tauri/icons/android");

/** Part du canevas occupée par le contenu (zone sûre adaptative ≈ 66 %). */
const CONTENT_RATIO = 0.64;
const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

function decodePng(buffer) {
	if (!buffer.subarray(0, 8).equals(PNG_SIGNATURE)) throw new Error("signature PNG invalide");

	const chunks = [];
	let offset = 8;
	while (offset + 8 <= buffer.length) {
		const length = buffer.readUInt32BE(offset);
		const type = buffer.toString("ascii", offset + 4, offset + 8);
		chunks.push({ type, data: buffer.subarray(offset + 8, offset + 8 + length) });
		offset += 12 + length;
		if (type === "IEND") break;
	}

	const ihdr = chunks.find((chunk) => chunk.type === "IHDR")?.data;
	if (!ihdr || ihdr.length < 13) throw new Error("chunk IHDR manquant");

	const width = ihdr.readUInt32BE(0);
	const height = ihdr.readUInt32BE(4);
	const bitDepth = ihdr[8];
	const colorType = ihdr[9];
	const interlace = ihdr[12];
	if (bitDepth !== 8 || colorType !== 6 || interlace !== 0) {
		throw new Error(`format PNG non supporté (bit=${bitDepth}, type=${colorType})`);
	}

	const idat = Buffer.concat(chunks.filter((chunk) => chunk.type === "IDAT").map((chunk) => chunk.data));
	const raw = zlib.inflateSync(idat);
	const stride = width * 4;
	const pixels = Buffer.alloc(stride * height);
	let previous = Buffer.alloc(stride);
	let cursor = 0;

	for (let y = 0; y < height; y += 1) {
		const filter = raw[cursor];
		cursor += 1;
		const line = Buffer.from(raw.subarray(cursor, cursor + stride));
		cursor += stride;

		for (let index = 0; index < stride; index += 1) {
			const left = index >= 4 ? line[index - 4] : 0;
			const up = previous[index];
			const upLeft = index >= 4 ? previous[index - 4] : 0;
			let value = line[index];

			if (filter === 1) value = (value + left) & 255;
			else if (filter === 2) value = (value + up) & 255;
			else if (filter === 3) value = (value + ((left + up) >> 1)) & 255;
			else if (filter === 4) {
				const estimate = left + up - upLeft;
				const distanceLeft = Math.abs(estimate - left);
				const distanceUp = Math.abs(estimate - up);
				const distanceUpLeft = Math.abs(estimate - upLeft);
				const predictor =
					distanceLeft <= distanceUp && distanceLeft <= distanceUpLeft
						? left
						: distanceUp <= distanceUpLeft
							? up
							: upLeft;
				value = (value + predictor) & 255;
			} else if (filter !== 0) {
				throw new Error(`filtre PNG inconnu : ${filter}`);
			}

			line[index] = value;
		}

		line.copy(pixels, y * stride);
		previous = line;
	}

	return { width, height, pixels };
}

let crcTable = null;
function crc32(buffer) {
	if (!crcTable) {
		crcTable = new Uint32Array(256);
		for (let n = 0; n < 256; n += 1) {
			let value = n;
			for (let k = 0; k < 8; k += 1) {
				value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
			}
			crcTable[n] = value >>> 0;
		}
	}

	let crc = 0xffffffff;
	for (const byte of buffer) crc = crcTable[(crc ^ byte) & 0xff] ^ (crc >>> 8);
	return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
	const typeBuffer = Buffer.from(type, "ascii");
	const length = Buffer.alloc(4);
	length.writeUInt32BE(data.length, 0);
	const crc = Buffer.alloc(4);
	crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0);
	return Buffer.concat([length, typeBuffer, data, crc]);
}

function encodePng(width, height, pixels) {
	const stride = width * 4;
	const raw = Buffer.alloc((stride + 1) * height);
	for (let y = 0; y < height; y += 1) {
		raw[y * (stride + 1)] = 0;
		pixels.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride);
	}

	const ihdr = Buffer.alloc(13);
	ihdr.writeUInt32BE(width, 0);
	ihdr.writeUInt32BE(height, 4);
	ihdr[8] = 8;
	ihdr[9] = 6;
	ihdr[10] = 0;
	ihdr[11] = 0;
	ihdr[12] = 0;

	return Buffer.concat([
		PNG_SIGNATURE,
		pngChunk("IHDR", ihdr),
		pngChunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
		pngChunk("IEND", Buffer.alloc(0)),
	]);
}

/** Boîte englobante des pixels visibles (alpha > seuil). */
function contentBounds({ width, height, pixels }) {
	let minX = width;
	let minY = height;
	let maxX = -1;
	let maxY = -1;

	for (let y = 0; y < height; y += 1) {
		for (let x = 0; x < width; x += 1) {
			if (pixels[(y * width + x) * 4 + 3] > 8) {
				if (x < minX) minX = x;
				if (x > maxX) maxX = x;
				if (y < minY) minY = y;
				if (y > maxY) maxY = y;
			}
		}
	}

	if (maxX < 0) return null;
	return { minX, minY, width: maxX - minX + 1, height: maxY - minY + 1 };
}

function padForeground(buffer, ratio = CONTENT_RATIO) {
	const image = decodePng(buffer);
	const bounds = contentBounds(image);
	if (!bounds) return encodePng(image.width, image.height, image.pixels);

	const { width: canvasWidth, height: canvasHeight, pixels } = image;
	const size = Math.min(canvasWidth, canvasHeight);
	const maxSide = Math.max(1, Math.round(size * ratio));
	const scale = Math.min(maxSide / bounds.width, maxSide / bounds.height);
	const targetWidth = Math.max(1, Math.round(bounds.width * scale));
	const targetHeight = Math.max(1, Math.round(bounds.height * scale));
	const offsetX = Math.floor((canvasWidth - targetWidth) / 2);
	const offsetY = Math.floor((canvasHeight - targetHeight) / 2);
	const output = Buffer.alloc(canvasWidth * canvasHeight * 4);

	for (let y = 0; y < targetHeight; y += 1) {
		const sourceY0 = bounds.minY + (y * bounds.height) / targetHeight;
		const sourceY1 = bounds.minY + ((y + 1) * bounds.height) / targetHeight;
		const y0 = Math.floor(sourceY0);
		const y1 = Math.max(y0 + 1, Math.ceil(sourceY1));

		for (let x = 0; x < targetWidth; x += 1) {
			const sourceX0 = bounds.minX + (x * bounds.width) / targetWidth;
			const sourceX1 = bounds.minX + ((x + 1) * bounds.width) / targetWidth;
			const x0 = Math.floor(sourceX0);
			const x1 = Math.max(x0 + 1, Math.ceil(sourceX1));

			let red = 0;
			let green = 0;
			let blue = 0;
			let alpha = 0;
			let count = 0;

			for (let sourceY = y0; sourceY < Math.min(y1, canvasHeight); sourceY += 1) {
				for (let sourceX = x0; sourceX < Math.min(x1, canvasWidth); sourceX += 1) {
					const index = (sourceY * canvasWidth + sourceX) * 4;
					const weight = pixels[index + 3] / 255;
					red += pixels[index] * weight;
					green += pixels[index + 1] * weight;
					blue += pixels[index + 2] * weight;
					alpha += pixels[index + 3];
					count += 1;
				}
			}

			const target = ((offsetY + y) * canvasWidth + offsetX + x) * 4;
			if (count > 0 && alpha > 0) {
				const weight = alpha / 255;
				output[target] = Math.round(red / weight);
				output[target + 1] = Math.round(green / weight);
				output[target + 2] = Math.round(blue / weight);
				output[target + 3] = Math.round(alpha / count);
			}
		}
	}

	return encodePng(canvasWidth, canvasHeight, output);
}

if (!existsSync(iconsRoot)) {
	console.error(`Dossier d'icônes Android introuvable : ${iconsRoot}`);
	process.exit(1);
}

let padded = 0;
for (const entry of readdirSync(iconsRoot)) {
	if (!entry.startsWith("mipmap-")) continue;
	const dir = path.join(iconsRoot, entry);
	if (!statSync(dir).isDirectory()) continue;
	const file = path.join(dir, "ic_launcher_foreground.png");
	if (!existsSync(file)) continue;

	writeFileSync(file, padForeground(readFileSync(file)));
	padded += 1;
}

if (padded === 0) {
	console.error(`Aucun ic_launcher_foreground.png dans ${iconsRoot}`);
	process.exit(1);
}

console.log(`Icône adaptative recadrée (${padded} densités, contenu à ${Math.round(CONTENT_RATIO * 100)} %).`);
