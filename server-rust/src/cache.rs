//! Cache mémoire avec TTL et déduplication des appels en vol.
//!
//! Reproduit `lib/cache.js` : une seule requête amont pour plusieurs appels
//! simultanés (pas de « cache stampede »), erreurs partagées mais jamais mises en cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::OnceCell;

use crate::error::{ApiError, Result};

const MAX_ENTRIES: usize = 800;

struct Entry {
	value: Arc<Value>,
	expires_at: Instant,
}

struct Inner {
	store: HashMap<String, Entry>,
	inflight: HashMap<String, Arc<OnceCell<std::result::Result<Arc<Value>, (u16, String)>>>>,
}

pub struct Cache {
	inner: Mutex<Inner>,
}

impl Cache {
	pub fn new() -> Self {
		Self {
			inner: Mutex::new(Inner {
				store: HashMap::new(),
				inflight: HashMap::new(),
			}),
		}
	}

	fn get_locked(inner: &Inner, key: &str) -> Option<Arc<Value>> {
		let entry = inner.store.get(key)?;
		if entry.expires_at <= Instant::now() {
			return None;
		}
		Some(entry.value.clone())
	}

	fn set_locked(inner: &mut Inner, key: &str, value: Arc<Value>, ttl_ms: u64) {
		inner.store.insert(
			key.to_string(),
			Entry {
				value,
				expires_at: Instant::now() + Duration::from_millis(ttl_ms),
			},
		);

		if inner.store.len() > MAX_ENTRIES {
			let now = Instant::now();
			let expired: Vec<String> = inner
				.store
				.iter()
				.filter(|(_, entry)| entry.expires_at <= now)
				.map(|(key, _)| key.clone())
				.collect();
			for key in expired {
				inner.store.remove(&key);
			}
		}
	}

	/// Renvoie la valeur en cache, ou exécute `producer` une seule fois.
	pub async fn cached<F, Fut>(&self, key: &str, ttl_ms: u64, producer: F) -> Result<Arc<Value>>
	where
		F: FnOnce() -> Fut,
		Fut: std::future::Future<Output = Result<Arc<Value>>>,
	{
		// 1. lecture cache + recherche d'un appel en vol, dans un seul verrou.
		let cell = {
			let mut inner = self.inner.lock().unwrap();
			if let Some(value) = Self::get_locked(&inner, key) {
				return Ok(value);
			}
			match inner.inflight.get(key) {
				Some(existing) => existing.clone(),
				None => {
					let created = Arc::new(OnceCell::new());
					inner.inflight.insert(key.to_string(), created.clone());
					created
				}
			}
		};

		// 2. exécution (une seule pour tous les appelants concurrents).
		let result = cell
			.get_or_init(|| async {
				producer().await.map_err(|error| (error.status, error.message))
			})
			.await;

		// 3. publication : le cache est rempli avant la sortie de la file en vol.
		let mut inner = self.inner.lock().unwrap();
		match result {
			Ok(value) => {
				Self::set_locked(&mut inner, key, value.clone(), ttl_ms);
				if inner
					.inflight
					.get(key)
					.is_some_and(|current| Arc::ptr_eq(current, &cell))
				{
					inner.inflight.remove(key);
				}
				Ok(value.clone())
			}
			Err((status, message)) => {
				if inner
					.inflight
					.get(key)
					.is_some_and(|current| Arc::ptr_eq(current, &cell))
				{
					inner.inflight.remove(key);
				}
				Err(ApiError::new(*status, message.clone()))
			}
		}
	}

	pub fn stats(&self) -> (usize, usize) {
		let inner = self.inner.lock().unwrap();
		(inner.store.len(), inner.inflight.len())
	}
}

static CACHE: std::sync::LazyLock<Cache> = std::sync::LazyLock::new(Cache::new);

/// Instance unique du cache (comme les imports partagés en Node).
pub fn cache() -> &'static Cache {
	&CACHE
}
