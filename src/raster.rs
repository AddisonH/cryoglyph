use cosmic_text::{CacheKey, FontSystem, SwashCache, SwashImage};

/// Rasterises a glyph for the atlas.
pub(crate) fn rasterize(
    font_system: &mut FontSystem,
    cache: &mut SwashCache,
    cache_key: CacheKey,
) -> Option<SwashImage> {
    cache.get_image_uncached(font_system, cache_key)
}
