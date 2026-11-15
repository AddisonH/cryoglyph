use cosmic_text::{CacheKey, CacheKeyFlags, FontSystem, Placement, SwashCache, SwashContent};
use zeno::{Fill, Format, Join, Mask, Origin, Stroke, Style, Vector};

use crate::{GlyphKey, Outline};

/// A rasterised glyph image.
pub(crate) struct Raster {
    pub(crate) content: SwashContent,
    pub(crate) placement: Placement,
    pub(crate) data: Vec<u8>,
}

const OUTLINE_STEPS_PER_PIXEL: f32 = 4.0;

/// The [`GlyphKey::outline`] of an outline `width` in physical pixels.
pub(crate) fn outline_key(width: f32) -> u16 {
    (width.clamp(0.0, Outline::MAX_WIDTH) * OUTLINE_STEPS_PER_PIXEL).round() as u16
}

/// Rasterises a glyph for the atlas.
pub(crate) fn rasterize(
    font_system: &mut FontSystem,
    cache: &mut SwashCache,
    key: GlyphKey,
) -> Option<Raster> {
    if key.outline == 0 {
        let image = cache.get_image_uncached(font_system, key.glyph)?;

        return Some(Raster {
            content: image.content,
            placement: image.placement,
            data: image.data,
        });
    }

    let commands = cache.get_outline_commands_uncached(font_system, key.glyph)?;
    let offset = subpixel_offset(key.glyph);
    let render = |style: Style| {
        let (data, mut placement) = Mask::new(&commands[..])
            .format(Format::Alpha)
            .origin(Origin::BottomLeft)
            .style(style)
            .offset(offset)
            .render_offset(offset)
            .render();

        // With a bottom-left origin zeno places the image by its bottom edge; the atlas, like
        // swash's images, places it by its top edge.
        placement.top += placement.height as i32;
        (data, placement)
    };

    let width = f32::from(key.outline) / OUTLINE_STEPS_PER_PIXEL;
    let mut stroke = Stroke::new(2.0 * width);
    // The default miter joins spike at sharp corners at small sizes.
    let _ = stroke.join(Join::Round);
    let (mut data, placement) = render(stroke.into());

    // A stroke alone is hollow inside wide stems; filling it in keeps the outlined glyph solid
    // under translucent text and makes its shadow the outlined shape.
    let (fill, fill_placement) = render(Fill::NonZero.into());
    let dx = fill_placement.left - placement.left;
    let dy = placement.top - fill_placement.top;

    for row in 0..fill_placement.height as i32 {
        for column in 0..fill_placement.width as i32 {
            let (x, y) = (column + dx, row + dy);

            if x < 0 || y < 0 || x >= placement.width as i32 || y >= placement.height as i32 {
                continue;
            }

            let target = &mut data[(y * placement.width as i32 + x) as usize];
            let coverage = fill[(row * fill_placement.width as i32 + column) as usize];
            *target = (*target).max(coverage);
        }
    }

    Some(Raster {
        content: SwashContent::Mask,
        placement,
        data,
    })
}

/// The offset cosmic-text renders a glyph at for its subpixel position.
fn subpixel_offset(glyph: CacheKey) -> Vector {
    if glyph.flags.contains(CacheKeyFlags::PIXEL_FONT) {
        Vector::new(
            glyph.x_bin.as_float().round(),
            glyph.y_bin.as_float().round(),
        )
    } else {
        Vector::new(glyph.x_bin.as_float(), glyph.y_bin.as_float())
    }
}
