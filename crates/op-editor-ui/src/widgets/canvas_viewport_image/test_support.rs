//! Shared reset helpers for image-pipeline tests.

use super::{
    data_url_cache, lock_decode_registry_for_tests, remote_images, DataUrlCache,
    RemoteImageRegistry,
};

pub(super) fn lock_statics() -> std::sync::MutexGuard<'static, ()> {
    let guard = lock_decode_registry_for_tests();
    clear_data_url_cache_for_tests();
    clear_remote_registry_for_tests();
    guard
}

pub(super) fn clear_data_url_cache_for_tests() {
    if let Ok(mut cache) = data_url_cache().lock() {
        *cache = DataUrlCache::new();
    }
}

pub(super) fn data_url_cache_len_for_tests() -> usize {
    data_url_cache()
        .lock()
        .map(|cache| cache.entries.len())
        .unwrap_or(0)
}

pub(super) fn clear_remote_registry_for_tests() {
    if let Ok(mut registry) = remote_images().lock() {
        *registry = RemoteImageRegistry::default();
    }
}
