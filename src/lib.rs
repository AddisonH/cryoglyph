//! Glyphon provides a simple way to render 2D text with [wgpu], [cosmic-text] and [etagere].
//!
//! [wpgu]: https://github.com/gfx-rs/wgpu
//! [cosmic-text]: https://github.com/pop-os/cosmic-text
//! [etagere]: https://github.com/nical/etagere

mod cache;
mod error;
mod raster;
mod text_atlas;
mod text_render;
mod viewport;

pub use cache::Cache;
pub use error::{PrepareError, RenderError};
pub use text_atlas::{ColorMode, TextAtlas};
pub use text_render::TextRenderer;
pub use viewport::Viewport;

use text_render::ContentType;

// Re-export all top-level types from `cosmic-text` for convenience.
#[doc(no_inline)]
pub use cosmic_text::{
    self, Action, Affinity, Attrs, AttrsList, AttrsOwned, Buffer, BufferLine, CacheKey, Color,
    Command, Cursor, Edit, Editor, Family, FamilyOwned, Font, FontSystem, LayoutCursor,
    LayoutGlyph, LayoutLine, LayoutRun, LayoutRunIter, Metrics, ShapeGlyph, ShapeLine, ShapeSpan,
    ShapeWord, Shaping, Stretch, Style, SubpixelBin, SwashCache, SwashContent, SwashImage, Weight,
    Wrap, fontdb,
};

use etagere::AllocId;

/// Identifies a glyph image in the atlas.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct GlyphKey {
    pub(crate) glyph: CacheKey,
    /// The outline width in quarters of a physical pixel, or 0 for the plain glyph.
    pub(crate) outline: u16,
}

/// Effects drawn beneath glyphs, in physical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphEffects {
    /// A border around each glyph.
    pub outline: Option<Outline>,
    /// A copy of each glyph, outline included, drawn at an offset.
    pub shadow: Option<Shadow>,
}

impl GlyphEffects {
    /// How far the effects reach past a glyph, in whole pixels.
    pub(crate) fn reach(&self) -> i32 {
        let outline = self
            .outline
            .map_or(0.0, |o| o.width.clamp(0.0, Outline::MAX_WIDTH));
        let shadow = self
            .shadow
            .map_or(0, |s| s.offset[0].abs().max(s.offset[1].abs()));
        outline.ceil() as i32 + shadow
    }
}

/// A border drawn around glyphs. Colour and bitmap glyphs, such as emoji, have none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outline {
    /// The colour of the border.
    pub color: Color,
    /// The width of the border, up to [`Outline::MAX_WIDTH`].
    pub width: f32,
}

impl Outline {
    /// The widest outline drawn; wider ones are narrowed to it.
    pub const MAX_WIDTH: f32 = 8.0;
}

/// A hard shadow: a copy of glyphs, in one colour, drawn at an offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    /// The colour of the shadow.
    pub color: Color,
    /// The offset of the shadow in whole pixels.
    pub offset: [i32; 2],
}

#[derive(Clone, Copy)]
pub(crate) enum GpuCacheStatus {
    InAtlas {
        x: u16,
        y: u16,
        content_type: ContentType,
    },
    SkipRasterization,
}

#[derive(Clone, Copy)]
pub(crate) struct GlyphDetails {
    width: u16,
    height: u16,
    gpu_cache: GpuCacheStatus,
    atlas_id: Option<AllocId>,
    top: i16,
    left: i16,
    /// The atlas `generation` in which this glyph was last touched by `prepare`. A glyph is
    /// considered in use for the current frame (and not evictable) while this equals the atlas's
    /// current `generation`.
    last_used: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct GlyphToRender {
    pos: [i32; 2],
    dim: [u16; 2],
    uv: [u16; 2],
    color: u32,
    content_type_with_srgb: [u16; 2],
    depth: f32,
}

/// The screen resolution to use when rendering text.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Resolution {
    /// The width of the screen in pixels.
    pub width: u32,
    /// The height of the screen in pixels.
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Params {
    screen_resolution: Resolution,
    _pad: [u32; 2],
}

/// Controls the visible area of the text. Any text outside of the visible area will be clipped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextBounds {
    /// The position of the left edge of the visible area.
    pub left: i32,
    /// The position of the top edge of the visible area.
    pub top: i32,
    /// The position of the right edge of the visible area.
    pub right: i32,
    /// The position of the bottom edge of the visible area.
    pub bottom: i32,
}

/// The default visible area doesn't clip any text.
impl Default for TextBounds {
    fn default() -> Self {
        Self {
            left: i32::MIN,
            top: i32::MIN,
            right: i32::MAX,
            bottom: i32::MAX,
        }
    }
}

/// A text area containing text to be rendered along with its overflow behavior.
pub struct TextArea<'a> {
    /// The buffer containing the text to be rendered.
    pub text: LayoutRunIter<'a>,
    /// The left edge of the buffer.
    pub left: f32,
    /// The top edge of the buffer.
    pub top: f32,
    /// The scaling to apply to the buffer.
    pub scale: f32,
    /// The visible bounds of the text area. This is used to clip the text and doesn't have to
    /// match the `left` and `top` values.
    pub bounds: TextBounds,
    /// The effects of the glyphs, indexed by each glyph's metadata; glyphs without an entry have
    /// none. Within the text area, all shadows are drawn first, then all outlines, then the
    /// glyphs, all at the glyph's depth.
    pub effects: &'a [GlyphEffects],
    // The default color of the text area.
    pub default_color: Color,
}
