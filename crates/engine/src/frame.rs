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

    /// Blend toward `other` by `k` (0..1).
    pub fn lerp(self, other: Color, k: f32) -> Self {
        let (a, b) = (self.0, other.0);
        Color(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * k))
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
    /// The light (e.g. the star), for the `*_shaded` draws. None: they draw
    /// unlit (full brightness).
    pub light: Option<Light>,
    /// A body reflecting the light (the nearest planet's day side), filling
    /// in the shade on the side facing it. None: no fill.
    pub reflector: Option<Reflector>,
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

/// A light source: a star. How bright it looks falls with the square of the
/// distance; the eye adapts part of the way (`EXPOSURE`), so a planet far out
/// is dim but not black and one close in is bright but not blinding.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// World position.
    pub position: DVec3,
    /// Its colour (the brightest channel 1).
    pub color: [f32; 3],
    /// Luminosity, relative: brightness 1 at `reference` metres from it.
    pub luminosity: f64,
    pub reference: f64,
}

/// A sphere reflecting the light: a planet or moon. What it gives a point
/// near it is the light falling on the ground beneath (by the sun's height
/// there), times the share it reflects (`albedo`), times how much of the sky
/// it fills ((R/d)²), coming from its direction, in its colour.
#[derive(Clone, Copy, Debug)]
pub struct Reflector {
    pub center: DVec3,
    pub radius: f64,
    pub albedo: f32,
    pub color: [f32; 3],
}

/// The share of a sphere's light a small face takes in: the sphere filling
/// `s` = sin²α of the sky (α its angular radius), the face's normal at
/// cosine `cos_b` to its direction. Far off it's a point (s·cos β); skimming
/// its surface it's a plane below ((1 + cos β)/2, a wall seeing half of it).
/// Blended smoothly between.
pub fn view_factor(cos_b: f32, s: f32) -> f32 {
    let c = 1.0 - (1.0 - s.min(1.0)).sqrt();
    s.min(1.0) * ((cos_b + c) / (1.0 + c)).max(0.0)
}

/// How much the eye adapts: perceived brightness goes as irradiance to this power.
pub const EXPOSURE: f32 = 0.3;

impl Light {
    /// Perceived brightness of its light at `p` (world), after adaptation.
    pub fn intensity_at(&self, p: DVec3) -> f32 {
        let d = p.distance(self.position).max(1.0);
        let irradiance = self.luminosity * (self.reference / d).powi(2);
        (irradiance as f32).powf(EXPOSURE).clamp(0.1, 1.6)
    }
}

/// Brightness of a face turned away from the light (see `Frame::model_shaded`).
pub const SHADE_AMBIENT: f32 = 0.06;
/// Brightness of an edge on the unlit side.
pub const LINE_AMBIENT: f32 = 0.25;

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
            light: None,
            reflector: None,
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

    /// Transformed model points, relative to the camera, and the model's
    /// center likewise.
    fn place(&self, model: &WireModel, t: &Transform) -> (Vec<Vec3>, Vec3) {
        let origin = t.position - self.camera.position;
        let pts = model.positions.iter().map(|&v| (origin + (t.rotation * v).as_dvec3() * t.scale).as_vec3()).collect();
        (pts, origin.as_vec3())
    }

    /// How lit a surface at `at` (camera-relative) facing `normal` is, 0..1
    /// (Lambert); 1 when there's no light.
    fn lambert(&self, at: Vec3, normal: Vec3) -> f32 {
        let Some(light) = self.light else { return 1.0 };
        let to_light = ((light.position - self.camera.position).as_vec3() - at).normalize_or_zero();
        normal.dot(to_light).max(0.0)
    }

    /// The light's colour times its brightness at `at` (camera-relative); white if none.
    fn light_at(&self, at: Vec3) -> [f32; 3] {
        let Some(light) = self.light else { return [1.0; 3] };
        let k = light.intensity_at(self.camera.position + at.as_dvec3());
        light.color.map(|c| c * k)
    }

    /// The reflector's light at `at` (camera-relative): the direction to it,
    /// how much of the sky it fills (sin² of its angular radius), and the
    /// share of the sun's light its ground sends back, with its colour times
    /// the sun's brightness here. None if there's none to speak of.
    fn fill_at(&self, at: Vec3) -> Option<(Vec3, f32, (f32, [f32; 3]))> {
        let (r, light) = (self.reflector?, self.light?);
        let p = self.camera.position + at.as_dvec3();
        let off = p - r.center;
        let d = off.length().max(r.radius);
        let up = off / off.length().max(1.0);
        // The ground beneath, lit by the sun's height over it.
        let day = up.dot((light.position - p).normalize_or_zero()).max(0.0) as f32;
        let s = (r.radius / d).powi(2) as f32;
        // The light off the ground (a share of the sun's), and its colour at the sun's brightness here.
        let base = r.albedo * day;
        let k = light.intensity_at(p);
        (base * s > 0.001).then(|| ((-up).as_vec3(), s, (base, [0, 1, 2].map(|j| r.color[j] * light.color[j] * k))))
    }

    /// Faces lit by `light`, flat-shaded (one tone per face): `base` times
    /// `SHADE_AMBIENT` in the dark up to full in direct light. Each face's
    /// normal is turned to face away from the model's center (the models are
    /// closed and roughly convex). Edges are dimmed on the far side by the
    /// light on their ends (a vertex facing away from the center), down to
    /// `LINE_AMBIENT` of `line`.
    fn shaded(&mut self, pts: &[Vec3], center: Vec3, model: &WireModel, edges: f32, line: impl Fn(u32) -> [f32; 4], fill: impl Fn(u32) -> [f32; 4]) {
        // Ambient, plus the star's light (in its colour, at its brightness
        // here), plus what the nearest planet reflects onto the side facing it.
        let light = self.light_at(center);
        // The planet's light: its direction and its strength here.
        let planet = self.fill_at(center);
        let lit = |c: [f32; 4], ambient: f32, k: f32, n: Vec3, light: [f32; 3]| {
            // How much of the planet's disc the face sees (see `view_factor`).
            // Through the eye's adaptation, as the star's light is (a tenth of
            // the sun's light still looks about half as bright).
            let k2 = planet.map_or(0.0, |(dir, s, (base, _))| ((base * view_factor(n.dot(dir), s)).powf(EXPOSURE) - 0.12).max(0.0) / 0.88);
            let f = planet.map_or([0.0; 3], |(_, _, (_, f))| f);
            let ch = |j: usize| c[j] * (ambient + (1.0 - ambient) * (k * light[j] + k2 * f[j]).min(1.6));
            [ch(0), ch(1), ch(2), c[3]]
        };
        for f in &model.faces {
            let [a, b, c] = [f[0], f[1], f[2]].map(|i| pts[i as usize]);
            let mid = (a + b + c) / 3.0;
            let mut n = (b - a).cross(c - a).normalize_or_zero();
            if n.dot(mid - center) < 0.0 {
                n = -n;
            }
            let k = self.lambert(mid, n);
            for &i in f {
                self.solids.push(Vertex { pos: pts[i as usize].to_array(), color: lit(fill(i), SHADE_AMBIENT, k, n, light) });
            }
        }
        if edges <= 0.0 {
            return;
        }
        for e in &model.edges {
            for &i in e {
                let p = pts[i as usize];
                let n = (p - center).normalize_or_zero();
                let k = self.lambert(p, n).sqrt();
                let [r, g, b, a] = lit(line(i), LINE_AMBIENT, k, n, light);
                self.lines.push(Vertex { pos: p.to_array(), color: [r * edges, g * edges, b * edges, a] });
            }
        }
    }

    /// A model lit by `light` (see `shaded`): edges in `line`, faces in `fill`.
    pub fn model_shaded(&mut self, model: &WireModel, t: &Transform, line: Color, fill: Color) {
        let (pts, center) = self.place(model, t);
        self.shaded(&pts, center, model, 1.0, |_| line.0, |_| fill.0);
    }

    /// Like `model_shaded`, with the edges at `edges` of their brightness
    /// (0: faces only), for fading detail with distance.
    pub fn model_shaded_faded(&mut self, model: &WireModel, t: &Transform, line: Color, fill: Color, edges: f32) {
        let (pts, center) = self.place(model, t);
        self.shaded(&pts, center, model, edges, |_| line.0, |_| fill.0);
    }

    /// A model in its own per-vertex colors, lit by `light`: edges at `line`
    /// times the vertex color (0: faces only, for fading detail with
    /// distance), faces at `fill` times it.
    pub fn model_colored_shaded(&mut self, model: &WireModel, t: &Transform, line: f32, fill: f32) {
        let (pts, center) = self.place(model, t);
        let tint = |k: f32| {
            move |i: u32| {
                let [r, g, b, a] = model.colors[i as usize];
                [r * k, g * k, b * k, a]
            }
        };
        self.shaded(&pts, center, model, line, tint(1.0), tint(fill));
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

    /// HUD line with a colour at each end (blended along it; alpha fades too).
    pub fn hud_line2(&mut self, a: Vec2, b: Vec2, ca: Color, cb: Color) {
        self.hud.push(Vertex { pos: [a.x, a.y, 0.0], color: ca.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, 0.0], color: cb.0 });
    }

    /// A filled disc that fades from `inner` at the center to `outer` at the
    /// rim (a glow), in HUD pixel coordinates.
    pub fn hud_glow(&mut self, center: Vec2, radius: f32, segments: u32, inner: Color, outer: Color) {
        let at = |i: u32| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            center + Vec2::new(a.cos(), a.sin()) * radius
        };
        for i in 0..segments {
            let (p, q) = (at(i), at(i + 1));
            for (v, c) in [(center, inner), (p, outer), (q, outer)] {
                self.hud_tris.push(Vertex { pos: [v.x, v.y, 0.0], color: c.0 });
            }
        }
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
