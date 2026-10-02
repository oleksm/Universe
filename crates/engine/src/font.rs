//! The HUD's typeface: Noto Sans Mono (see `assets/`), its printable ASCII
//! rasterised once into an atlas (a coverage mask) at `RASTER` times the
//! HUD's layout pixels, drawn as textured quads that keep the layout's
//! monospaced grid (`GLYPH` wide, centred in it). One corner of the atlas is
//! solid: plain rectangles sample it, so text and panels share one queue
//! and keep their order.

use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

/// Atlas pixels per HUD layout pixel (crisp up to about three times the
/// layout's size on screen).
pub const RASTER: f32 = 3.0;
/// The type's size in layout pixels (as ab_glyph scales it: the line's
/// height, ascender to descender): capitals about 8 high, filling the grid.
pub const EM: f32 = 15.0;
/// From the top of a text line to its baseline (layout pixels).
pub const BASELINE: f32 = 8.3;

/// Where a glyph lies in the atlas (uv, 0..1) and on the line (layout
/// pixels from the pen at the baseline: left, top, width, height).
#[derive(Clone, Copy, Default, Debug)]
pub struct Glyph {
    pub uv: [f32; 4],
    pub at: [f32; 4],
}

pub struct Atlas {
    pub width: u32,
    pub height: u32,
    /// Coverage, one byte a pixel.
    pub pixels: Vec<u8>,
    /// Printable ASCII (32..127), and where the solid patch is (uv).
    pub glyphs: [Glyph; 96 + EXTRA.len()],
    pub solid: [f32; 2],
    /// A glyph's advance (layout pixels).
    pub advance: f32,
}

pub fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(build)
}

/// Beyond printable ASCII: in the atlas's last row (before the solid patch).
pub const EXTRA: [char; 6] = ['°', '–', '·', '²', '³', '×'];

/// A character's place among the glyphs, if the atlas has it.
pub fn glyph_index(ch: char) -> Option<usize> {
    match ch {
        ' '..='~' => Some(ch as usize - 32),
        _ => EXTRA.iter().position(|&c| c == ch).map(|k| 96 + k),
    }
}

fn build() -> Atlas {
    let font = FontRef::try_from_slice(include_bytes!("../assets/NotoSansMono-Medium.ttf")).expect("the HUD font loads");
    let scale = PxScale::from(EM * RASTER);
    let scaled = font.as_scaled(scale);
    // A grid of cells, 16 across, each big enough for any glyph (plus a margin).
    let cell = (EM * RASTER * 1.5).ceil() as u32;
    let (cols, rows) = (16u32, 7u32);
    let (width, height) = (cols * cell, rows * cell);
    let mut pixels = vec![0u8; (width * height) as usize];
    let mut glyphs = [Glyph::default(); 96 + EXTRA.len()];
    for (k, ch) in (32u8..128).map(char::from).chain(EXTRA).enumerate() {
        let (cx, cy) = ((k as u32 % cols) * cell, (k as u32 / cols) * cell);
        // (The pen at the cell's left, the baseline a third of the way up from its foot.)
        let base = (cx as f32 + cell as f32 * 0.2, cy as f32 + cell as f32 * 0.72);
        let g = font.glyph_id(ch).with_scale_and_position(scale, ab_glyph::point(base.0, base.1));
        let Some(outline) = font.outline_glyph(g) else { continue };
        let b = outline.px_bounds();
        outline.draw(|x, y, c| {
            let (px, py) = (b.min.x as i64 + x as i64, b.min.y as i64 + y as i64);
            if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                pixels[(py as u32 * width + px as u32) as usize] = (c.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });
        glyphs[k] = Glyph {
            uv: [b.min.x / width as f32, b.min.y / height as f32, b.max.x / width as f32, b.max.y / height as f32],
            at: [(b.min.x - base.0) / RASTER, (b.min.y - base.1) / RASTER, b.width() / RASTER, b.height() / RASTER],
        };
    }
    // The solid patch: the last cell's corner.
    let (sx, sy) = ((cols - 1) * cell, (rows - 1) * cell);
    for y in sy..sy + 4 {
        for x in sx..sx + 4 {
            pixels[(y * width + x) as usize] = 255;
        }
    }
    let advance = scaled.h_advance(font.glyph_id('M')) / RASTER;
    Atlas { width, height, pixels, glyphs, solid: [(sx as f32 + 2.0) / width as f32, (sy as f32 + 2.0) / height as f32], advance }
}
