use glam::{DVec3, Vec2, Vec3, Vec4Swizzles};

use crate::camera::Camera;
use crate::model::{Transform, WireModel};

/// Linear RGBA color, written straight to the (non-sRGB) framebuffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const BLACK: Color = Color([0.0, 0.0, 0.0, 1.0]);
    pub const WHITE: Color = Color([1.0, 1.0, 1.0, 1.0]);

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Color([r, g, b, 1.0])
    }

    /// `0xRRGGBB`.
    pub const fn hex(v: u32) -> Self {
        Color::rgb(
            ((v >> 16) & 0xff) as f32 / 255.0,
            ((v >> 8) & 0xff) as f32 / 255.0,
            (v & 0xff) as f32 / 255.0,
        )
    }

    pub fn scale(self, k: f32) -> Self {
        let [r, g, b, a] = self.0;
        Color([r * k, g * k, b * k, a])
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub pos: [f32; 3],
    pub color: [f32; 4],
}

/// Immediate-mode draw list for one frame.
///
/// World-space input is `f64`; it is converted to camera-relative `f32` on submission.
pub struct Frame {
    pub camera: Camera,
    pub clear: Color,
    /// Low-res scene resolution.
    scene_size: Vec2,
    /// HUD layer resolution (a multiple of the scene's).
    size: Vec2,
    pub(crate) sky: Vec<Vertex>,
    pub(crate) solids: Vec<Vertex>,
    pub(crate) lines: Vec<Vertex>,
    pub(crate) points: Vec<Vertex>,
    pub(crate) hud_tris: Vec<Vertex>,
    pub(crate) hud: Vec<Vertex>,
}

/// Width and height of one character cell of the HUD font, in pixels.
pub const GLYPH: f32 = 8.0;

impl Frame {
    /// Lines, triangles and points in the draw lists (scene and HUD).
    pub(crate) fn counts(&self) -> (u32, u32, u32) {
        let n = |v: &Vec<Vertex>| v.len() as u32;
        (n(&self.lines) / 2 + n(&self.hud) / 2, n(&self.solids) / 3 + n(&self.hud_tris) / 3, n(&self.points) + n(&self.sky))
    }

    pub(crate) fn new(camera: Camera, scene_size: Vec2, hud_size: Vec2) -> Self {
        Self {
            camera,
            clear: Color::BLACK,
            scene_size,
            size: hud_size,
            sky: Vec::new(),
            solids: Vec::new(),
            lines: Vec::new(),
            points: Vec::new(),
            hud_tris: Vec::new(),
            hud: Vec::new(),
        }
    }

    /// Size of the HUD layer in pixels: the coordinate space for all `hud_*`/`text` calls
    /// and for `project`.
    pub fn size(&self) -> Vec2 {
        self.size
    }

    fn rel(&self, p: DVec3) -> [f32; 3] {
        self.camera.relative(p).to_array()
    }

    /// Scene pixels per radian at the screen center; multiply by an angular size to get pixels.
    pub fn pixels_per_radian(&self) -> f32 {
        self.scene_size.y * 0.5 / (self.camera.fov_y * 0.5).tan()
    }

    /// Approximate on-screen radius of a sphere, in scene (low-res) pixels.
    pub fn projected_radius(&self, center: DVec3, radius: f64) -> f32 {
        let d = (center - self.camera.position).length();
        if d <= radius {
            return f32::INFINITY;
        }
        (radius / d) as f32 * self.pixels_per_radian()
    }

    /// World position to HUD pixel coordinates (see `size`), or `None` if behind the camera.
    pub fn project(&self, p: DVec3) -> Option<Vec2> {
        let clip = self.camera.view_proj(self.size.x / self.size.y) * self.camera.relative(p).extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.xy() / clip.w;
        Some(Vec2::new((ndc.x + 1.0) * 0.5 * self.size.x, (1.0 - ndc.y) * 0.5 * self.size.y))
    }

    /// A 1-pixel point in the world, depth-tested against solids.
    pub fn point(&mut self, p: DVec3, color: Color) {
        let pos = self.rel(p);
        self.points.push(Vertex { pos, color: color.0 });
    }

    /// A 1-pixel point infinitely far away in direction `dir` (stars, nebulae...).
    pub fn sky_point(&mut self, dir: Vec3, color: Color) {
        self.sky.push(Vertex { pos: dir.to_array(), color: color.0 });
    }

    pub fn line(&mut self, a: DVec3, b: DVec3, color: Color) {
        let (a, b) = (self.rel(a), self.rel(b));
        self.lines.push(Vertex { pos: a, color: color.0 });
        self.lines.push(Vertex { pos: b, color: color.0 });
    }

    /// Solid triangle. Writes depth, so it hides lines behind it.
    pub fn triangle(&mut self, a: DVec3, b: DVec3, c: DVec3, color: Color) {
        for p in [a, b, c] {
            let pos = self.rel(p);
            self.solids.push(Vertex { pos, color: color.0 });
        }
    }

    /// Circle outline of `radius` around `center`, lying in the plane with `normal`.
    pub fn circle(&mut self, center: DVec3, normal: DVec3, radius: f64, segments: u32, color: Color) {
        let n = normal.normalize();
        let u = n.any_orthonormal_vector();
        let v = n.cross(u);
        let at = |i: u32| {
            let a = i as f64 / segments as f64 * std::f64::consts::TAU;
            center + (u * a.cos() + v * a.sin()) * radius
        };
        for i in 0..segments {
            self.line(at(i), at(i + 1), color);
        }
    }

    /// Draw a wireframe model: `fill` faces occlude, `edges` are drawn in `line`.
    pub fn model(&mut self, model: &WireModel, t: &Transform, line: Color, fill: Color) {
        let origin = t.position - self.camera.position;
        let pts: Vec<[f32; 3]> = model
            .positions
            .iter()
            .map(|&v| (origin + (t.rotation * v).as_dvec3() * t.scale).as_vec3().to_array())
            .collect();
        for f in &model.faces {
            for &i in f {
                self.solids.push(Vertex { pos: pts[i as usize], color: fill.0 });
            }
        }
        for e in &model.edges {
            for &i in e {
                self.lines.push(Vertex { pos: pts[i as usize], color: line.0 });
            }
        }
    }

    /// Line with a color at each end (blended along it).
    pub fn line2(&mut self, a: DVec3, b: DVec3, ca: Color, cb: Color) {
        let (a, b) = (self.rel(a), self.rel(b));
        self.lines.push(Vertex { pos: a, color: ca.0 });
        self.lines.push(Vertex { pos: b, color: cb.0 });
    }

    /// Solid triangle with a color per corner.
    pub fn triangle3(&mut self, p: [DVec3; 3], c: [Color; 3]) {
        for i in 0..3 {
            let pos = self.rel(p[i]);
            self.solids.push(Vertex { pos, color: c[i].0 });
        }
    }

    /// Draw a model using its own per-vertex colors: edges at `line` times the
    /// vertex color, occluding faces at `fill` times it (a dark tint).
    pub fn model_colored(&mut self, model: &WireModel, t: &Transform, line: f32, fill: f32) {
        let origin = t.position - self.camera.position;
        let pts: Vec<[f32; 3]> = model
            .positions
            .iter()
            .map(|&v| (origin + (t.rotation * v).as_dvec3() * t.scale).as_vec3().to_array())
            .collect();
        let tint = |i: u32, k: f32| {
            let [r, g, b, a] = model.colors[i as usize];
            [r * k, g * k, b * k, a]
        };
        for f in &model.faces {
            for &i in f {
                self.solids.push(Vertex { pos: pts[i as usize], color: tint(i, fill) });
            }
        }
        for e in &model.edges {
            for &i in e {
                self.lines.push(Vertex { pos: pts[i as usize], color: tint(i, line) });
            }
        }
    }

    /// Line in HUD pixel coordinates (origin top-left, low-res pixels).
    /// Endpoints are snapped to pixel centers so lines stay crisp.
    pub fn hud_line(&mut self, a: Vec2, b: Vec2, color: Color) {
        let (a, b) = (a.floor() + 0.5, b.floor() + 0.5);
        self.hud.push(Vertex { pos: [a.x, a.y, 0.0], color: color.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, 0.0], color: color.0 });
    }

    /// Filled rectangle in HUD pixel coordinates.
    pub fn hud_rect(&mut self, pos: Vec2, size: Vec2, color: Color) {
        let (a, b) = (pos, pos + size);
        for [x, y] in [[a.x, a.y], [b.x, a.y], [b.x, b.y], [a.x, a.y], [b.x, b.y], [a.x, b.y]] {
            self.hud_tris.push(Vertex { pos: [x, y, 0.0], color: color.0 });
        }
    }

    /// Rectangle outline in HUD pixel coordinates.
    pub fn hud_box(&mut self, pos: Vec2, size: Vec2, color: Color) {
        let s = size - 1.0;
        let c = [pos, pos + Vec2::new(s.x, 0.0), pos + s, pos + Vec2::new(0.0, s.y)];
        for i in 0..4 {
            self.hud_line(c[i], c[(i + 1) % 4], color);
        }
        self.hud_rect(pos + s, Vec2::ONE, color); // close the last corner pixel
    }

    /// Ellipse outline in HUD pixel coordinates.
    pub fn hud_ellipse(&mut self, center: Vec2, radii: Vec2, segments: u32, color: Color) {
        let at = |i: u32| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            center + Vec2::new(a.cos(), a.sin()) * radii
        };
        for i in 0..segments {
            self.hud_line(at(i), at(i + 1), color);
        }
    }

    /// Draw text with the 8x8 bitmap font; `\n` starts a new line. Returns the end position.
    pub fn text(&mut self, pos: Vec2, text: &str, color: Color) -> Vec2 {
        let origin = pos.floor();
        let mut cursor = origin;
        for ch in text.chars() {
            if ch == '\n' {
                cursor = Vec2::new(origin.x, cursor.y + GLYPH + 2.0);
                continue;
            }
            let glyph = font8x8::legacy::BASIC_LEGACY.get(ch as usize).unwrap_or(&font8x8::legacy::BASIC_LEGACY[b'?' as usize]);
            for (row, bits) in glyph.iter().enumerate() {
                // Merge horizontal runs of set bits into single quads.
                let mut col = 0;
                while col < 8 {
                    if bits & (1 << col) == 0 {
                        col += 1;
                        continue;
                    }
                    let start = col;
                    while col < 8 && bits & (1 << col) != 0 {
                        col += 1;
                    }
                    let p = cursor + Vec2::new(start as f32, row as f32);
                    self.hud_rect(p, Vec2::new((col - start) as f32, 1.0), color);
                }
            }
            cursor.x += GLYPH;
        }
        cursor
    }

    /// Text with a solid background box behind it, for readability over the scene.
    pub fn text_boxed(&mut self, pos: Vec2, text: &str, color: Color, background: Color) {
        let size = text_size(text);
        self.hud_rect(pos - 2.0, size + 4.0, background);
        self.text(pos, text, color);
    }
}

/// Pixel size of a (possibly multi-line) string in the HUD font.
pub fn text_size(text: &str) -> Vec2 {
    let lines = text.split('\n');
    let (mut w, mut h) = (0usize, 0usize);
    for line in lines {
        w = w.max(line.chars().count());
        h += 1;
    }
    Vec2::new(w as f32 * GLYPH, h as f32 * (GLYPH + 2.0) - 2.0)
}
