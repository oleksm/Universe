use glam::{DVec3, Vec2, Vec3, Vec4Swizzles};

use crate::camera::Camera;
use crate::model::{Mesh, Transform};

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
    /// Meshes to draw this frame (transformed and lit on the GPU).
    pub(crate) meshes: Vec<MeshDraw>,
    /// Meshes that hide anchored HUD (see `occluder`).
    pub(crate) occluders: Vec<MeshDraw>,
    /// 1 while drawing anchored HUD (carried in the HUD vertices' z).
    hud_z: f32,
}

/// One mesh draw: the mesh, and its instance data.
pub(crate) struct MeshDraw {
    pub mesh: Mesh,
    pub instance: Instance,
    /// Draw its edges too.
    pub edges: bool,
}

/// Per-draw data for the mesh shader: the model's rotation × scale (columns)
/// and camera-relative position; edge and face tints; the star's direction
/// (w: the faces' ambient) and its colour × brightness here (w: the edges'
/// ambient); the reflecting planet's direction (w: how much sky it fills)
/// and colour (w: the share of sunlight it sends back).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Instance {
    pub c0: [f32; 4],
    pub c1: [f32; 4],
    pub c2: [f32; 4],
    pub t: [f32; 4],
    pub line_tint: [f32; 4],
    pub fill_tint: [f32; 4],
    pub light_dir: [f32; 4],
    pub light_color: [f32; 4],
    pub refl_dir: [f32; 4],
    pub refl_color: [f32; 4],
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
        let mesh_lines: usize = self.meshes.iter().filter(|m| m.edges).map(|m| m.mesh.edges.len()).sum();
        let mesh_tris: usize = self.meshes.iter().map(|m| m.mesh.faces.len()).sum();
        (n(&self.lines) / 2 + n(&self.hud) / 2 + mesh_lines as u32, n(&self.solids) / 3 + n(&self.hud_tris) / 3 + mesh_tris as u32, n(&self.points) + n(&self.sky))
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
            meshes: Vec::new(),
            occluders: Vec::new(),
            hud_z: 0.0,
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
    /// Mark a mesh (drawn as usual too) as hiding what the HUD marks behind
    /// it: HUD drawn inside `anchored` is cut out wherever it covers it.
    pub fn occluder(&mut self, mesh: &Mesh, t: &Transform) {
        self.mesh(mesh, t, [1.0; 4], [1.0; 4], false, false);
        if let Some(d) = self.meshes.pop() {
            self.occluders.push(d);
        }
    }

    /// HUD drawn in `f` marks things in the world (labels, brackets, contact
    /// boxes): an occluder in front of the camera (our hull) hides it, pixel
    /// by pixel, as it would hide what it marks. Instruments (gunsight,
    /// flight path) stay on top.
    pub fn anchored(&mut self, f: impl FnOnce(&mut Frame)) {
        let before = self.hud_z;
        self.hud_z = 1.0;
        f(self);
        self.hud_z = before;
    }

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
    fn fill_at(&self, p: DVec3) -> Option<(Vec3, f32, (f32, [f32; 3]))> {
        let (r, light) = (self.reflector?, self.light?);
        let off = p - r.center;
        // The reflector itself (or anything at its heart) isn't lit by it.
        if off.length() < r.radius * 0.5 {
            return None;
        }
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

    /// A mesh, lit (flat faces, edges dimmed on the far side, see the mesh
    /// shader): edges in `line`, faces in `fill`.
    pub fn model_shaded(&mut self, mesh: &Mesh, t: &Transform, line: Color, fill: Color) {
        self.mesh(mesh, t, line.0, fill.0, true, true);
    }

    /// Like `model_shaded`, with the edges at `edges` of their brightness
    /// (0: faces only), for fading detail with distance.
    pub fn model_shaded_faded(&mut self, mesh: &Mesh, t: &Transform, line: Color, fill: Color, edges: f32) {
        let [r, g, b, a] = line.0;
        self.mesh(mesh, t, [r * edges, g * edges, b * edges, a], fill.0, edges > 0.0, true);
    }

    /// A mesh in its own per-vertex colors, lit: edges at `line` times the
    /// vertex color (0: faces only), faces at `fill` times it.
    pub fn model_colored_shaded(&mut self, mesh: &Mesh, t: &Transform, line: f32, fill: f32) {
        self.mesh(mesh, t, [line, line, line, 1.0], [fill, fill, fill, 1.0], line > 0.0, true);
    }

    /// An unlit mesh: faces in `fill` occlude, edges in `line`.
    pub fn model(&mut self, mesh: &Mesh, t: &Transform, line: Color, fill: Color) {
        self.mesh(mesh, t, line.0, fill.0, true, false);
    }

    /// An unlit mesh in its own per-vertex colors, times `line` and `fill`.
    pub fn model_colored(&mut self, mesh: &Mesh, t: &Transform, line: f32, fill: f32) {
        self.mesh(mesh, t, [line, line, line, 1.0], [fill, fill, fill, 1.0], true, false);
    }

    /// Queue a mesh draw: where it is (camera-relative), its tints, and the
    /// light on it here, for the GPU to transform and light.
    fn mesh(&mut self, mesh: &Mesh, t: &Transform, line: [f32; 4], fill: [f32; 4], edges: bool, lit: bool) {
        let origin = t.position - self.camera.position;
        let m = glam::Mat3::from_quat(t.rotation) * t.scale as f32;
        let at = origin.as_vec3();
        let mut inst = Instance {
            c0: m.x_axis.extend(0.0).to_array(),
            c1: m.y_axis.extend(0.0).to_array(),
            c2: m.z_axis.extend(0.0).to_array(),
            t: at.extend(0.0).to_array(),
            line_tint: line,
            fill_tint: fill,
            // Unlit: full brightness (ambient 1).
            light_dir: [0.0, 0.0, 0.0, 1.0],
            light_color: [0.0, 0.0, 0.0, 1.0],
            refl_dir: [0.0; 4],
            refl_color: [0.0; 4],
        };
        if lit && let Some(light) = self.light {
            let dir = (light.position - t.position).normalize_or_zero().as_vec3();
            let c = self.light_at(at);
            inst.light_dir = dir.extend(SHADE_AMBIENT).to_array();
            inst.light_color = [c[0], c[1], c[2], LINE_AMBIENT];
            if let Some((d, s, (base, c))) = self.fill_at(t.position) {
                inst.refl_dir = d.extend(s).to_array();
                inst.refl_color = [c[0], c[1], c[2], base];
            }
        }
        self.meshes.push(MeshDraw { mesh: mesh.clone(), instance: inst, edges });
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

    /// Line in HUD pixel coordinates (origin top-left, low-res pixels).
    /// Endpoints are snapped to pixel centers so lines stay crisp.
    pub fn hud_line(&mut self, a: Vec2, b: Vec2, color: Color) {
        let (a, b) = (a.floor() + 0.5, b.floor() + 0.5);
        self.hud.push(Vertex { pos: [a.x, a.y, self.hud_z], color: color.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, self.hud_z], color: color.0 });
    }

    /// HUD line with a colour at each end (blended along it; alpha fades too).
    pub fn hud_line2(&mut self, a: Vec2, b: Vec2, ca: Color, cb: Color) {
        self.hud.push(Vertex { pos: [a.x, a.y, self.hud_z], color: ca.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, self.hud_z], color: cb.0 });
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
                self.hud_tris.push(Vertex { pos: [v.x, v.y, self.hud_z], color: c.0 });
            }
        }
    }

    /// Filled rectangle in HUD pixel coordinates.
    pub fn hud_rect(&mut self, pos: Vec2, size: Vec2, color: Color) {
        let (a, b) = (pos, pos + size);
        for [x, y] in [[a.x, a.y], [b.x, a.y], [b.x, b.y], [a.x, a.y], [b.x, b.y], [a.x, b.y]] {
            self.hud_tris.push(Vertex { pos: [x, y, self.hud_z], color: color.0 });
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
