import { config } from "../config.js";

const hits = new Map();
const WINDOW = config.rateLimit.windowMs;
const MAX = config.rateLimit.max;

/** Rate limit memoire simple (fenetre glissante) pour proteger les API amont. */
export const rateLimit = (req, res, next) => {
	const now = Date.now();
	const ip = req.ip || req.socket?.remoteAddress || "unknown";
	const entry = hits.get(ip);

	if (!entry || entry.resetAt <= now) {
		hits.set(ip, { count: 1, resetAt: now + WINDOW });
		res.setHeader("x-ratelimit-remaining", String(MAX - 1));
		return next();
	}

	entry.count += 1;
	res.setHeader("x-ratelimit-remaining", String(Math.max(0, MAX - entry.count)));

	if (entry.count > MAX) {
		res.setHeader("retry-after", String(Math.ceil((entry.resetAt - now) / 1000)));
		return res.status(429).json({ error: { status: 429, message: "Trop de requetes, merci de patienter." } });
	}

	if (hits.size > 5000) {
		for (const [key, value] of hits) if (value.resetAt <= now) hits.delete(key);
	}

	return next();
};
