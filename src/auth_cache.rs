//! In-memory API-key → device cache so hot ingest does not hit Postgres every message.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::auth::AuthDevice;

#[derive(Clone)]
struct CacheEntry {
    device: AuthDevice,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct AuthCache {
    inner: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
}

impl AuthCache {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            ttl: Duration::from_secs(ttl_secs.max(5)),
        }
    }

    pub async fn get(&self, key_hash: &str) -> Option<AuthDevice> {
        let guard = self.inner.read().await;
        let entry = guard.get(key_hash)?;
        if entry.expires_at > Instant::now() {
            Some(entry.device.clone())
        } else {
            None
        }
    }

    pub async fn put(&self, key_hash: String, device: AuthDevice) {
        let mut guard = self.inner.write().await;
        guard.insert(
            key_hash,
            CacheEntry {
                device,
                expires_at: Instant::now() + self.ttl,
            },
        );
        // Soft bound so long runs do not grow forever.
        if guard.len() > 50_000 {
            let now = Instant::now();
            guard.retain(|_, e| e.expires_at > now);
        }
    }

    pub async fn invalidate_device(&self, device_id: Uuid) {
        let mut guard = self.inner.write().await;
        guard.retain(|_, e| e.device.id != device_id);
    }

    pub async fn invalidate_hash(&self, key_hash: &str) {
        let mut guard = self.inner.write().await;
        guard.remove(key_hash);
    }
}
