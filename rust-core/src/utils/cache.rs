use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub genre: Option<String>,
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
    pub file_size: u64,
    pub modified_time: Option<u64>,
}

impl Metadata {
    pub fn new(file_size: u64) -> Self {
        Self {
            title: None,
            artist: None,
            album: None,
            year: None,
            track_number: None,
            genre: None,
            duration: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
            bitrate: None,
            file_size,
            modified_time: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.title.is_some() || self.artist.is_some()
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    metadata: Metadata,
    accessed: Instant,
}

pub struct MetadataCache {
    cache: Arc<Mutex<HashMap<PathBuf, CacheEntry>>>,
    max_size: usize,
    ttl: Duration,
    hits: Arc<Mutex<u64>>,
    misses: Arc<Mutex<u64>>,
}

impl MetadataCache {
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            max_size,
            ttl,
            hits: Arc::new(Mutex::new(0)),
            misses: Arc::new(Mutex::new(0)),
        }
    }

    pub fn get(&self, path: &PathBuf) -> Option<Metadata> {
        let mut cache = self.cache.lock();
        
        if let Some(entry) = cache.get_mut(path) {
            if entry.accessed.elapsed() < self.ttl {
                entry.accessed = Instant::now();
                *self.hits.lock() += 1;
                return Some(entry.metadata.clone());
            } else {
                cache.remove(path);
            }
        }

        *self.misses.lock() += 1;
        None
    }

    pub fn put(&self, path: PathBuf, metadata: Metadata) {
        let mut cache = self.cache.lock();

        if cache.len() >= self.max_size {
            self.evict_lru(&mut cache);
        }

        cache.insert(path, CacheEntry {
            metadata,
            accessed: Instant::now(),
        });
    }

    pub fn remove(&self, path: &PathBuf) {
        let mut cache = self.cache.lock();
        cache.remove(path);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
    }

    pub fn contains(&self, path: &PathBuf) -> bool {
        let cache = self.cache.lock();
        cache.contains_key(path)
    }

    pub fn size(&self) -> usize {
        let cache = self.cache.lock();
        cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.lock();
        let now = Instant::now();
        
        cache.retain(|_, entry| {
            now.duration_since(entry.accessed) < self.ttl
        });
    }

    fn evict_lru(&self, cache: &mut HashMap<PathBuf, CacheEntry>) {
        if let Some(lru_key) = cache.iter()
            .min_by_key(|(_, entry)| entry.accessed)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&lru_key);
        }
    }

    pub fn get_stats(&self) -> CacheStats {
        let cache = self.cache.lock();
        let hits = *self.hits.lock();
        let misses = *self.misses.lock();
        let total = hits + misses;
        
        CacheStats {
            size: cache.len(),
            max_size: self.max_size,
            hits,
            misses,
            hit_rate: if total > 0 { hits as f64 / total as f64 } else { 0.0 },
        }
    }

    pub fn reset_stats(&self) {
        *self.hits.lock() = 0;
        *self.misses.lock() = 0;
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

impl Default for MetadataCache {
    fn default() -> Self {
        Self::new(10000, Duration::from_secs(3600))
    }
}

pub struct AsyncMetadataCache {
    cache: Arc<MetadataCache>,
}

impl AsyncMetadataCache {
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(MetadataCache::new(max_size, ttl)),
        }
    }

    pub async fn get(&self, path: PathBuf) -> Option<Metadata> {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            cache.get(&path)
        }).await.ok().flatten()
    }

    pub async fn put(&self, path: PathBuf, metadata: Metadata) {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            cache.put(path, metadata);
        }).await.ok();
    }

    pub async fn remove(&self, path: PathBuf) {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            cache.remove(&path);
        }).await.ok();
    }

    pub async fn cleanup_expired(&self) {
        let cache = Arc::clone(&self.cache);
        tokio::task::spawn_blocking(move || {
            cache.cleanup_expired();
        }).await.ok();
    }

    pub fn get_cache(&self) -> &MetadataCache {
        &self.cache
    }
}

impl Clone for AsyncMetadataCache {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
        }
    }
}
