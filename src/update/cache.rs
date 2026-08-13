use super::{StableVersion, UpdateCache};
use crate::storage::atomic_write;
use anyhow::{Context, Result};
use std::{fs, io::Read, path::Path};

const MAX_CACHE_BYTES: u64 = 4 * 1024;

pub(super) fn load_cache(path: &Path) -> Option<UpdateCache> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CACHE_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_CACHE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_CACHE_BYTES as usize {
        return None;
    }
    let cache = serde_json::from_slice::<UpdateCache>(&bytes).ok()?;
    (cache.schema_version == 1 && cache.latest.parse::<StableVersion>().is_ok()).then_some(cache)
}

pub(super) fn write_cache(path: &Path, cache: &UpdateCache) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(cache).context("failed to serialize update cache")?;
    atomic_write(path, &bytes).with_context(|| format!("failed to save {}", path.display()))
}
