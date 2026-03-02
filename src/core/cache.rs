// ABOUTME: MessagePack cache for processed diff results.
// ABOUTME: Keyed by repo+base+head+file to avoid re-parsing on repeated queries.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Cache stored on disk as MessagePack.
///
/// Loaded once at construction; each `put` flushes the updated store to disk.
pub struct Cache {
    path: PathBuf,
    store: Store,
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    entries: HashMap<String, Vec<u8>>,
}

impl Cache {
    pub fn new(path: PathBuf) -> Self {
        let store = std::fs::read(&path)
            .ok()
            .and_then(|bytes| rmp_serde::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, store }
    }

    /// Default cache location: ~/.cache/pasua/cache.msgpack
    pub fn default_path() -> PathBuf {
        let mut p = dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache"));
        p.push("pasua");
        p.push("cache.msgpack");
        p
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = rmp_serde::to_vec(&self.store)?;
        std::fs::write(&self.path, bytes)?;
        Ok(())
    }

    pub fn key(repo: &Path, base: &str, head: &str, file: &str) -> String {
        format!("{}|{base}|{head}|{file}", repo.display())
    }

    pub fn get<T: for<'de> Deserialize<'de>>(
        &self,
        repo: &Path,
        base: &str,
        head: &str,
        file: &str,
    ) -> Option<T> {
        let key = Self::key(repo, base, head, file);
        self.store
            .entries
            .get(&key)
            .and_then(|bytes| rmp_serde::from_slice(bytes).ok())
    }

    pub fn put<T: Serialize>(
        &mut self,
        repo: &Path,
        base: &str,
        head: &str,
        file: &str,
        value: &T,
    ) -> Result<()> {
        let key = Self::key(repo, base, head, file);
        let bytes = rmp_serde::to_vec(value)?;
        self.store.entries.insert(key, bytes);
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trip() {
        let dir = TempDir::new().unwrap();
        let cache_path = dir.path().join("cache.msgpack");
        let mut cache = Cache::new(cache_path);

        let repo = Path::new("/repo");
        let value = vec!["hello".to_string(), "world".to_string()];
        cache
            .put(repo, "main", "feature", "foo.go", &value)
            .unwrap();

        let got: Vec<String> = cache.get(repo, "main", "feature", "foo.go").unwrap();
        assert_eq!(got, value);
    }

    #[test]
    fn miss_returns_none() {
        let dir = TempDir::new().unwrap();
        let cache = Cache::new(dir.path().join("cache.msgpack"));
        let got: Option<String> = cache.get(Path::new("/repo"), "a", "b", "c.go");
        assert!(got.is_none());
    }

    #[test]
    fn multiple_puts_accumulate() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.msgpack");
        let mut cache = Cache::new(path.clone());
        let repo = Path::new("/repo");

        cache.put(repo, "a", "b", "f1.go", &1u32).unwrap();
        cache.put(repo, "a", "b", "f2.go", &2u32).unwrap();

        // Both entries survive in a fresh load from disk
        let c2 = Cache::new(path);
        assert_eq!(c2.get::<u32>(repo, "a", "b", "f1.go"), Some(1));
        assert_eq!(c2.get::<u32>(repo, "a", "b", "f2.go"), Some(2));
    }
}
