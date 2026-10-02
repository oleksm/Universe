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

/// A HUD triangle's corner: where (layout pixels), what of the font atlas
/// (a glyph's coverage, or its solid patch for plain fills), what colour.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
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
    /// Spheres that can come between the light and what it lights (planets,
    /// moons: centre, radius), each hiding the share of its disc it covers.
    pub eclipsers: Vec<(DVec3, f64)>,
    /// Shadows cast by meshes on meshes, out to this far from the eye
    /// (metres; 0: none). See `no_shadow`.
    pub shadow_reach: f64,
    casts: bool,
    /// The surface meshes are drawn with now (see `Instance::material`, `with_surface`).
    surface: [f32; 4],
    /// Low-res scene resolution.
    scene_size: Vec2,
    /// HUD layer resolution (a multiple of the scene's).
    size: Vec2,
    pub(crate) sky: Vec<Vertex>,
    pub(crate) solids: Vec<Vertex>,
    pub(crate) lines: Vec<Vertex>,
    /// Lines drawn with the front layer's meshes (see `in_front`).
    pub(crate) front_lines: Vec<Vertex>,
    pub(crate) points: Vec<Vertex>,
    /// Lights' glows: discs facing the eye, adding light (see `glow`).
    pub(crate) glows: Vec<Vertex>,
    pub(crate) hud_tris: Vec<HudVertex>,
    pub(crate) hud: Vec<Vertex>,
    /// Meshes to draw this frame (transformed and lit on the GPU).
    pub(crate) meshes: Vec<MeshDraw>,
    /// Meshes drawn over everything, the HUD included (see `in_front`).
    pub(crate) front: Vec<MeshDraw>,
    in_front: bool,
}

/// One mesh draw: the mesh, and its instance data.
pub(crate) struct MeshDraw {
    pub mesh: Mesh,
    pub instance: Instance,
    /// Draw its edges too.
    pub edges: bool,
    /// It casts shadows (see `Frame::no_shadow`); how far it reaches from its
    /// origin (metres, scaled).
    pub casts: bool,
    pub reach: f32,
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
    /// Its surface: how much it glints in the light (0 matte .. 1), how
    /// tight the glint (a power: higher, sharper), how much it glows itself.
    pub material: [f32; 4],
}

/// A light source: a star. How bright it looks falls with the square of the
/// distance; the eye adapts only part of the way (`EXPOSURE`), so a planet
/// far out is dark and one close in is bright — near the star, blown out.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// World position.
    pub position: DVec3,
    /// Its colour (the brightest channel 1).
    pub color: [f32; 3],
    /// Luminosity, relative: brightness 1 at `reference` metres from it.
    pub luminosity: f64,
    pub reference: f64,
    /// Its radius (metres): the disc a body can cover part of.
    pub radius: f64,
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

/// How much the eye adapts: perceived brightness goes as irradiance to this
/// power (1: not at all; 0: completely).
pub const EXPOSURE: f32 = 0.45;
/// The brightest light a face takes (several times the light at 1 AU from
/// a sun-like star: near a star, colours wash out to white).
pub const MAX_LIGHT: f32 = 4.0;

impl Light {
    /// Perceived brightness of its light at `p` (world), after adaptation.
    pub fn intensity_at(&self, p: DVec3) -> f32 {
        (self.irradiance_at(p) as f32).powf(EXPOSURE).clamp(0.02, MAX_LIGHT)
    }

    /// Irradiance at `p` (world), against `reference` metres from a sun-like star (1).
    pub fn irradiance_at(&self, p: DVec3) -> f64 {
        let d = p.distance(self.position).max(1.0);
        self.luminosity * (self.reference / d).powi(2)
    }
}

/// The share of a disc of (angular) radius `a` that a disc of radius `b`,
/// `d` from it centre to centre, covers: none apart, all within, the lens
/// between.
pub fn disc_covered(a: f64, b: f64, d: f64) -> f64 {
    if d >= a + b {
        return 0.0;
    }
    if d <= b - a {
        return 1.0;
    }
    if d <= a - b {
        return (b / a).powi(2);
    }
    let lens = a * a * ((d * d + a * a - b * b) / (2.0 * d * a)).clamp(-1.0, 1.0).acos()
        + b * b * ((d * d + b * b - a * a) / (2.0 * d * b)).clamp(-1.0, 1.0).acos()
        - 0.5 * ((-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b)).max(0.0).sqrt();
    (lens / (std::f64::consts::PI * a * a)).clamp(0.0, 1.0)
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
        (n(&self.lines) / 2 + n(&self.hud) / 2 + mesh_lines as u32, n(&self.solids) / 3 + self.hud_tris.len() as u32 / 3 + mesh_tris as u32, n(&self.points) + n(&self.sky))
    }

    pub(crate) fn new(camera: Camera, scene_size: Vec2, hud_size: Vec2) -> Self {
        Self {
            camera,
            clear: Color::BLACK,
            light: None,
            reflector: None,
            eclipsers: Vec::new(),
            shadow_reach: 0.0,
            casts: true,
            surface: [0.0, 16.0, 0.0, 0.0],
            scene_size,
            size: hud_size,
            sky: Vec::new(),
            solids: Vec::new(),
            lines: Vec::new(),
            front_lines: Vec::new(),
            points: Vec::new(),
            glows: Vec::new(),
            hud_tris: Vec::new(),
            hud: Vec::new(),
            meshes: Vec::new(),
            front: Vec::new(),
            in_front: false,
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
    /// Meshes drawn in `f` go on top of everything: the scene and the HUD
    /// (with their own depth among themselves). For what's nearest the eye,
    /// like our own ship seen from just behind it: nothing shows through it.
    pub fn in_front(&mut self, f: impl FnOnce(&mut Frame)) {
        let before = std::mem::replace(&mut self.in_front, true);
        f(self);
        self.in_front = before;
    }

    /// A light's glow at `at` (world): a soft disc facing the eye, `radius`
    /// metres across (and never less than `least` HUD pixels, so a lamp far
    /// off still shows), adding `light` (linear, may be over 1: it blooms in
    /// the tone curve). Hidden behind what's solid; hides nothing.
    pub fn glow(&mut self, at: DVec3, radius: f64, light: [f32; 3], least: f32) {
        let rel = at - self.camera.position;
        let d = rel.length();
        if d < 1e-3 {
            return;
        }
        let per_px = d / self.pixels_per_radian().max(1e-3) as f64;
        let r = radius.max(least as f64 * per_px);
        // (Facing the eye: across its right and up.)
        let q = self.camera.orientation.as_dquat();
        let (right, up) = (q * DVec3::X * r, q * DVec3::Y * r);
        let centre = Vertex { pos: rel.as_vec3().to_array(), color: [light[0], light[1], light[2], 1.0] };
        let rim = |k: usize| {
            let a = k as f64 / 12.0 * std::f64::consts::TAU;
            Vertex { pos: (rel + right * a.cos() + up * a.sin()).as_vec3().to_array(), color: [0.0, 0.0, 0.0, 0.0] }
        };
        for k in 0..12 {
            self.glows.extend([centre, rim(k), rim(k + 1)]);
        }
    }

    /// A triangle of added light (a glow's shape of your own: an
    /// atmosphere's rim), its colour at each corner (linear, may be over 1).
    pub fn glow_triangle(&mut self, p: [DVec3; 3], light: [[f32; 3]; 3]) {
        for k in 0..3 {
            let pos = self.rel(p[k]);
            self.glows.push(Vertex { pos, color: [light[k][0], light[k][1], light[k][2], 1.0] });
        }
    }

    /// Meshes drawn in `f` have this surface: `glint` (0 matte .. 1 polished
    /// metal), `sharp` (its power: 8 broad .. 80 tight), `glow` (lit by itself).
    pub fn with_surface(&mut self, glint: f32, sharp: f32, glow: f32, f: impl FnOnce(&mut Frame)) {
        let before = std::mem::replace(&mut self.surface, [glint, sharp, glow, 0.0]);
        f(self);
        self.surface = before;
    }

    /// Meshes drawn in `f` cast no shadows (a planet's globe: its night is
    /// its own shading, and its eclipses are `eclipsers`).
    pub fn no_shadow(&mut self, f: impl FnOnce(&mut Frame)) {
        let before = std::mem::replace(&mut self.casts, false);
        f(self);
        self.casts = before;
    }

    /// How much of the light's disc is in sight from `at` (world), 0..1:
    /// what the eclipsers leave of it. (A sphere holding `at` doesn't count:
    /// its own night is its shading.)
    pub fn sun_visible(&self, at: DVec3) -> f64 {
        let Some(light) = self.light else { return 1.0 };
        let to_sun = light.position - at;
        let ds = to_sun.length();
        if ds <= light.radius {
            return 1.0;
        }
        let a = (light.radius / ds).asin();
        let mut seen = 1.0;
        for &(c, r) in &self.eclipsers {
            let to = c - at;
            let d = to.length();
            if d <= r || d >= ds || to.dot(to_sun) <= 0.0 {
                continue;
            }
            let b = (r / d).asin();
            let sep = to.angle_between(to_sun);
            seen *= 1.0 - disc_covered(a, b, sep);
        }
        seen
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
        self.line2(a, b, color, color);
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
            material: self.surface,
        };
        if lit && let Some(light) = self.light {
            let dir = (light.position - t.position).normalize_or_zero().as_vec3();
            // (What a planet or moon leaves of the sun here.)
            let seen = self.sun_visible(t.position) as f32;
            let c = self.light_at(at).map(|c| c * seen);
            inst.light_dir = dir.extend(SHADE_AMBIENT).to_array();
            inst.light_color = [c[0], c[1], c[2], LINE_AMBIENT];
            if let Some((d, s, (base, c))) = self.fill_at(t.position) {
                inst.refl_dir = d.extend(s).to_array();
                inst.refl_color = [c[0], c[1], c[2], base];
            }
        }
        let d = MeshDraw { mesh: mesh.clone(), instance: inst, edges, casts: self.casts, reach: mesh.radius() * t.scale as f32 };
        if self.in_front { self.front.push(d) } else { self.meshes.push(d) }
    }

    /// Line with a color at each end (blended along it).
    pub fn line2(&mut self, a: DVec3, b: DVec3, ca: Color, cb: Color) {
        let (a, b) = (self.rel(a), self.rel(b));
        let lines = if self.in_front { &mut self.front_lines } else { &mut self.lines };
        lines.push(Vertex { pos: a, color: ca.0 });
        lines.push(Vertex { pos: b, color: cb.0 });
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
        self.hud.push(Vertex { pos: [a.x, a.y, 0.0], color: color.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, 0.0], color: color.0 });
    }

    /// HUD line with a colour at each end (blended along it; alpha fades too).
    pub fn hud_line2(&mut self, a: Vec2, b: Vec2, ca: Color, cb: Color) {
        self.hud.push(Vertex { pos: [a.x, a.y, 0.0], color: ca.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, 0.0], color: cb.0 });
    }

    /// A HUD triangle, a colour at each corner (blended across it).
    pub fn hud_triangle_colored(&mut self, p: [Vec2; 3], c: [Color; 3]) {
        let solid = crate::font::atlas().solid;
        for k in 0..3 {
            self.hud_tris.push(HudVertex { pos: [p[k].x, p[k].y], uv: solid, color: c[k].0 });
        }
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
                self.hud_tris.push(HudVertex { pos: [v.x, v.y], uv: crate::font::atlas().solid, color: c.0 });
            }
        }
    }

    /// Filled rectangle in HUD pixel coordinates.
    pub fn hud_rect(&mut self, pos: Vec2, size: Vec2, color: Color) {
        let (a, b) = (pos, pos + size);
        for [x, y] in [[a.x, a.y], [b.x, a.y], [b.x, b.y], [a.x, a.y], [b.x, b.y], [a.x, b.y]] {
            self.hud_tris.push(HudVertex { pos: [x, y], uv: crate::font::atlas().solid, color: color.0 });
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

    /// Draw text (the HUD typeface, on its monospaced grid); `\n` starts a
    /// new line. Returns the end position.
    pub fn text(&mut self, pos: Vec2, text: &str, color: Color) -> Vec2 {
        let atlas = crate::font::atlas();
        let origin = pos;
        let mut cursor = origin;
        // (Each glyph centred in its cell of the grid.)
        let inset = (GLYPH - atlas.advance) * 0.5;
        for ch in text.chars() {
            if ch == '\n' {
                cursor = Vec2::new(origin.x, cursor.y + GLYPH + 2.0);
                continue;
            }
            let k = (ch as usize).wrapping_sub(32);
            let g = atlas.glyphs.get(k).copied().unwrap_or(atlas.glyphs[(b'?' - 32) as usize]);
            if g.at[2] > 0.0 {
                let p = cursor + Vec2::new(inset + g.at[0], crate::font::BASELINE + g.at[1]);
                let (a, b) = (p, p + Vec2::new(g.at[2], g.at[3]));
                let (u0, v0, u1, v1) = (g.uv[0], g.uv[1], g.uv[2], g.uv[3]);
                let v = |x: f32, y: f32, u: f32, w: f32| HudVertex { pos: [x, y], uv: [u, w], color: color.0 };
                self.hud_tris.extend([v(a.x, a.y, u0, v0), v(b.x, a.y, u1, v0), v(b.x, b.y, u1, v1), v(a.x, a.y, u0, v0), v(b.x, b.y, u1, v1), v(a.x, b.y, u0, v1)]);
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
