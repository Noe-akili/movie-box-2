const store = new Map();
const inflight = new Map();
const MAX_ENTRIES = 800;

export const cacheGet = (key) => {
	const entry = store.get(key);
	if (!entry) return null;
	if (entry.expiresAt <= Date.now()) {
		store.delete(key);
		return null;
	}
	return entry.value;
};

export const cacheSet = (key, value, ttlMs) => {
	store.set(key, { value, expiresAt: Date.now() + ttlMs });

	if (store.size > MAX_ENTRIES) {
		const now = Date.now();
		for (const [entryKey, entry] of store) {
			if (entry.expiresAt <= now) store.delete(entryKey);
		}
	}

	return value;
};

/**
 * Renvoie la valeur en cache si elle existe, sinon l'execute une seule fois
 * (les appels concurrents partagent la meme promesse = pas de "cache stampede").
 */
export const cached = async (key, ttlMs, producer) => {
	const hit = cacheGet(key);
	if (hit !== null) return hit;

	const pending = inflight.get(key);
	if (pending) return pending;

	const task = (async () => {
		try {
			const value = await producer();
			cacheSet(key, value, ttlMs);
			return value;
		} finally {
			inflight.delete(key);
		}
	})();

	inflight.set(key, task);
	return task;
};

export const cacheStats = () => ({ entries: store.size, inflight: inflight.size });
