//! Decoded-raster + typeface cache accessors on `NativeBackend`.
//!
//! The caches themselves are fields on `NativeBackend`; these are the
//! install / lookup / eviction entry points every image paint goes
//! through, split out of `skia.rs` to keep it under the repo's 800-line
//! cap.

use super::*;

impl NativeBackend {
    /// Return an already-rasterized image without decoding.
    pub fn raster_image(&mut self, id: u64) -> Option<skia_safe::Image> {
        self.image_cache_tick += 1;
        let tick = self.image_cache_tick;
        let hit = self.image_cache.get_mut(&id)?;
        hit.last_used = tick;
        Some(hit.image.clone())
    }

    /// Generation of the installed-raster set — see the field's note.
    /// A composition cached across frames stamps this and drops or
    /// re-renders itself when it no longer matches.
    pub fn raster_generation(&self) -> u64 {
        self.raster_generation
    }

    /// Install worker-decoded pixels into the paint-side LRU.
    /// `covers_edge_px` records how sharp this raster is, so a later
    /// zoom-in can tell that it needs a finer decode.
    pub fn install_raster_image(&mut self, id: u64, image: skia_safe::Image, covers_edge_px: u32) {
        self.image_cache_tick += 1;
        self.raster_generation = self.raster_generation.wrapping_add(1);
        let bytes = (image.width().max(0) as usize)
            .saturating_mul(image.height().max(0) as usize)
            .saturating_mul(4);
        if let Some(old) = self.image_cache.remove(&id) {
            self.image_cache_bytes = self.image_cache_bytes.saturating_sub(old.bytes);
        }
        self.image_cache_bytes = self.image_cache_bytes.saturating_add(bytes);
        self.image_cache.insert(
            id,
            ImageCacheEntry {
                image,
                bytes,
                last_used: self.image_cache_tick,
                covers_edge_px,
            },
        );
        self.evict_images_over(IMAGE_CACHE_BYTE_BUDGET, IMAGE_CACHE_MAX_ENTRIES);
    }

    /// Readiness hook used by `NativeFrameBackend`; never decodes.
    /// A cached raster that is too coarse for the requested size reports
    /// `false` so paint queues a sharper decode — the existing raster
    /// still draws in the meantime (paint falls back to `raster_image`),
    /// so zooming in sharpens progressively instead of blanking.
    pub fn image_decoded(&mut self, id: u64, encoded: &[u8], max_edge_px: u32) -> bool {
        let _ = encoded;
        self.image_cache
            .get(&id)
            .is_some_and(|entry| entry.covers_edge_px >= max_edge_px)
    }

    /// Whether any raster for `id` is resident, at any sharpness.
    pub fn image_resident(&self, id: u64) -> bool {
        self.image_cache.contains_key(&id)
    }

    /// Evict least-recently-used image entries until the cache fits
    /// both `byte_budget` and `max_entries`. Separated from
    /// [`Self::install_raster_image`] so tests can exercise eviction with
    /// small budgets.
    pub(super) fn evict_images_over(&mut self, byte_budget: usize, max_entries: usize) {
        while self.image_cache_bytes > byte_budget || self.image_cache.len() > max_entries {
            let Some((&oldest, _)) = self
                .image_cache
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
            else {
                break;
            };
            if let Some(entry) = self.image_cache.remove(&oldest) {
                self.image_cache_bytes = self.image_cache_bytes.saturating_sub(entry.bytes);
            }
        }
    }

    /// Number of cached image entries — test accessor.
    #[cfg(test)]
    pub(crate) fn image_cache_len(&self) -> usize {
        self.image_cache.len()
    }

    #[cfg(test)]
    pub(crate) fn family_typeface_cache_len(&self) -> usize {
        self.font_resolver.cache_len()
    }
}
