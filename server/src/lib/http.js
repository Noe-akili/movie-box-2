import { ApiError } from "./errors.js";

const DEFAULT_TIMEOUT = 12_000;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export const buildUrl = (base, params = {}) => {
	const url = new URL(base);

	Object.entries(params).forEach(([key, value]) => {
		if (value === undefined || value === null || value === "") return;
		url.searchParams.set(key, String(value));
	});

	return url.toString();
};

/** fetch + JSON + timeout + retry, avec des erreurs API propres. */
export const fetchJson = async (url, { timeoutMs = DEFAULT_TIMEOUT, retries = 1, headers = {} } = {}) => {
	let lastError = null;

	for (let attempt = 0; attempt <= retries; attempt += 1) {
		const controller = new AbortController();
		const timer = setTimeout(() => controller.abort(), timeoutMs);

		try {
			const response = await fetch(url, {
				signal: controller.signal,
				headers: { accept: "application/json", "user-agent": "movie-box/1.0", ...headers },
			});
			const body = await response.text();

			if (!response.ok) {
				// Wikimedia et consorts limitent le debit : on respecte Retry-After avant de reessayer.
				const retryAfter = Number(response.headers.get("retry-after") || 0);
				if (response.status === 429 && attempt < retries) {
					await sleep(Math.min(5_000, Math.max(700, retryAfter * 1_000)));
					continue;
				}

				throw new ApiError(response.status === 404 ? 404 : 502, `Service amont indisponible (${response.status})`, {
					url,
				});
			}

			return body ? JSON.parse(body) : null;
		} catch (error) {
			lastError = error;
			if (error instanceof ApiError && error.status === 404) break;
			if (attempt < retries) await sleep(200 * (attempt + 1));
		} finally {
			clearTimeout(timer);
		}
	}

	if (lastError instanceof ApiError) throw lastError;
	throw new ApiError(504, "Delai depasse lors de l'appel au service amont", { url });
};

export const fetchBuffer = async (url, { timeoutMs = 20_000 } = {}) => {
	const controller = new AbortController();
	const timer = setTimeout(() => controller.abort(), timeoutMs);

	try {
		const response = await fetch(url, { signal: controller.signal, redirect: "follow" });
		if (!response.ok) throw new ApiError(502, `Service amont indisponible (${response.status})`, { url });
		return {
			contentType: response.headers.get("content-type") || "application/octet-stream",
			buffer: Buffer.from(await response.arrayBuffer()),
			status: response.status,
		};
	} catch (error) {
		if (error instanceof ApiError) throw error;
		throw new ApiError(504, "Delai depasse lors de l'appel au service amont", { url });
	} finally {
		clearTimeout(timer);
	}
};
