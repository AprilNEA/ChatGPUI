// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::RwLock;

use super::model::Model;

/// Cache entry with timestamp
struct CacheEntry {
    models: Vec<Model>,
    fetched_at: Instant,
}

/// Global model cache for all providers
pub struct ModelCache {
    /// Provider ID -> cached models
    cache: RwLock<HashMap<String, CacheEntry>>,
    /// Cache TTL (time to live)
    ttl: Duration,
}

impl ModelCache {
    /// Create a new model cache with the given TTL
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    /// Create with default TTL of 1 hour
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(3600))
    }

    /// Get cached models for a provider, if valid
    pub async fn get(&self, provider_id: &str) -> Option<Vec<Model>> {
        let cache = self.cache.read().await;
        cache.get(provider_id).and_then(|entry| {
            if entry.fetched_at.elapsed() < self.ttl {
                Some(entry.models.clone())
            } else {
                None
            }
        })
    }

    /// Store models in cache
    pub async fn set(&self, provider_id: &str, models: Vec<Model>) {
        let mut cache = self.cache.write().await;
        cache.insert(
            provider_id.to_string(),
            CacheEntry {
                models,
                fetched_at: Instant::now(),
            },
        );
    }

    /// Invalidate cache for a specific provider
    pub async fn invalidate(&self, provider_id: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(provider_id);
    }

    /// Invalidate all caches
    pub async fn invalidate_all(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Global model cache instance
static MODEL_CACHE: std::sync::OnceLock<Arc<ModelCache>> = std::sync::OnceLock::new();

/// Get the global model cache
pub fn get_model_cache() -> Arc<ModelCache> {
    MODEL_CACHE
        .get_or_init(|| Arc::new(ModelCache::with_default_ttl()))
        .clone()
}

/// Fetch models for a provider, using cache if available
pub async fn fetch_models_cached(
    provider_id: &str,
    fetcher: impl std::future::Future<Output = Result<Vec<Model>>>,
) -> Result<Vec<Model>> {
    let cache = get_model_cache();

    // Check cache first
    if let Some(models) = cache.get(provider_id).await {
        tracing::debug!("Using cached models for provider {}", provider_id);
        return Ok(models);
    }

    // Fetch from provider
    let models = fetcher.await?;

    // Update cache
    cache.set(provider_id, models.clone()).await;
    tracing::debug!("Cached models for provider {}", provider_id);

    Ok(models)
}
