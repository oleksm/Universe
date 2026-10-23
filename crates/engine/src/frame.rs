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

/// Material alpha policy for perspective canvas geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CanvasAlpha {
    Opaque,
    Mask(f32),
    Blend,
}

/// A HUD triangle's corner: where (layout pixels), what of the font atlas
/// (a glyph's coverage, or its solid patch for plain fills), what colour.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub depth: [f32; 2], // eye distance, near plane; zero for UI
    pub alpha: [f32; 2], // mode (opaque/mask/blend), cutoff
    pub scene: u32, // independent canvas depth scope (CPU batching)
}

/// What the renderer draws of what it can: each on by default, each off
/// alone, to see what it brings (and trace a difference to it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Graphics {
    /// The sun's shadows (ships, stations, rocks on each other and the ground).
    pub shadows: bool,
    /// Models' textures (colour, roughness, metalness): off, their materials' plain values.
    pub textures: bool,
    /// Models' normal maps (fine relief: seams, rivets, panel edges).
    pub normal_maps: bool,
    /// Models' ambient occlusion (seams and corners light from round about doesn't reach).
    pub occlusion: bool,
    /// What glows of itself (lamps, windows, engine glow).
    pub emission: bool,
    /// Glossy highlights: the sun's glint and the planet's reflection.
    pub specular: bool,
    /// The light a nearby planet or moon sends back.
    pub planet_light: bool,
    /// The film curve (AgX): off, light straight to the screen, clipped.
    pub tone_map: bool,
    /// A world's clouds (and their shadows, and their cache's work). Off by default for now (the
    /// user's call, 2026-10-07: clouds aren't the priority yet).
    pub clouds: bool,
}

impl Default for Graphics {
    fn default() -> Self {
        Graphics { shadows: true, textures: true, normal_maps: true, occlusion: true, emission: true, specular: true, planet_light: true, tone_map: true, clouds: false }
    }
}

/// Immediate-mode draw list for one frame.
///
/// World-space input is `f64`; it is converted to camera-relative `f32` on submission.
pub struct Frame {
    keepalive: Vec<std::sync::Arc<dyn Send + Sync>>,
    pub camera: Camera,
    pub projection: crate::viewport::Projection,
    pub(crate) views: Vec<(u64, crate::viewport::PixelRect, Box<Frame>, usize)>,
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
    /// The light every lit face gets whatever faces it (0 to 1; `SHADE_AMBIENT`
    /// in the world: space is dark): raised in a studio.
    pub ambient: f32,
    /// How far off what the eye is looking at is (m; 0: nothing in
    /// particular): a tight shadow cascade is drawn round it.
    pub shadow_focus: f64,
    /// The environment is a studio's (soft boxes), not space round the eye.
    pub studio: bool,
    /// Lit faces shaded on the screen's colours, as a map shades relief (see `ViewShade`); None: by the scene's light.
    pub view_shade: Option<ViewShade>,
    /// An unsharp mask on the screen's colours, as the planet lab's views have: its amount (0: none) over a
    /// one-pixel blur (3×3 binomial), the scene only (not the HUD or the front layer).
    pub sharpen: f32,
    /// A planet's colour and lit normal by direction (see `PlanetCube`), for meshes whose vertices carry their
    /// directions in `data` (w: `PLANET_CUBE_DATA`). None: a blank one.
    pub planet_cube: Option<std::sync::Arc<crate::worldmaps::PlanetCube>>,
    /// Ask how much of the sun is hidden by what's drawn: a point just this
    /// side of it (world) and its disc's radius on screen (pixels). The
    /// answer, a frame or two on: `sun_seen`.
    pub sun_probe: Option<(DVec3, f32)>,
    /// Shadows cast by meshes on meshes, out to this far from the eye
    /// (metres; 0: none). See `no_shadow`.
    pub shadow_reach: f64,
    /// What's drawn of what can be (see `Graphics`).
    pub graphics: Graphics,
    /// How what's drawn now is drawn (see `DrawState`): set for a closure by `in_front`, `with_globe`...
    state: DrawState,
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
    /// Glows drawn with the front layer (see `in_front`).
    pub(crate) front_glows: Vec<Vertex>,
    pub(crate) hud_tris: Vec<HudVertex>,
    canvas_scene: u32,
    pub(crate) hud: Vec<Vertex>,
    /// Meshes to draw this frame (transformed and lit on the GPU).
    pub(crate) meshes: Vec<MeshDraw>,
    /// Transparent, depth-tested meshes after opaque terrain/models; never depth writers or casters.
    pub(crate) overlays: Vec<MeshDraw>,
    pub(crate) mesh_uploads: Vec<Mesh>,
    pub(crate) mesh_releases: Vec<u64>,
    /// Pooled meshes to draw this frame (see `pool_group`).
    pub(crate) pool: crate::pool::PoolFrame,
    /// Meshes drawn over everything, the HUD included (see `in_front`).
    pub(crate) front: Vec<MeshDraw>,
    /// Textured, physically based models to draw this frame (see `model_pbr`).
    pub(crate) pbr: Vec<PbrDraw>,
    /// Globe maps drawn this frame (instances name them by place here + 1).
    pub(crate) globe_maps: Vec<std::sync::Arc<crate::model::GlobeMap>>,
    /// A world's full-resolution maps, and the globe map (its id) of the world they're for.
    pub(crate) world_maps: Option<(std::sync::Arc<crate::worldmaps::WorldMaps>, u64, glam::DVec3, f64)>,
    /// That world's turn (its frame to the scene's) and its clouds now: month, El Niño, seconds
    /// (wrapped), 1 (see `world_clouds`).
    pub(crate) world_turn: glam::DQuat,
    pub(crate) world_clouds: [f32; 4],
}

/// How what's drawn now is drawn: one state, changed for a closure and put back after it (`Frame::with_state`).
#[derive(Clone, Copy, Debug)]
struct DrawState {
    /// Over everything, the HUD included (see `in_front`), or in the scene.
    in_front: bool,
    /// Meshes cast shadows (see `no_shadow`); they cast into the ground's own cascade (see `ground_shadow`).
    casts: bool,
    ground: bool,
    /// The surface meshes are drawn with (see `Instance::material`, `with_surface`).
    surface: [f32; 4],
    /// A world's globe surfacing them (see `with_globe`): its map's place + 1, kind, relief, brightness; where
    /// its vertices are; a patch's fine grain origin.
    globe: [f32; 4],
    globe_at: [f32; 4],
    globe_micro: [f32; 4],
    /// The air over a globe (see `with_air`): its depths packed, its shell (m).
    globe_air: [f32; 2],
}

impl Default for DrawState {
    fn default() -> Self {
        Self { in_front: false, casts: true, ground: false, surface: [0.0, 16.0, 0.0, 0.0], globe: [0.0; 4], globe_at: [0.0, 0.0, 0.0, 1.0], globe_micro: [0.0; 4], globe_air: [0.0; 2] }
    }
}

/// A group of pooled draws this frame (see `Frame::pool_group`).
#[derive(Clone, Copy, Debug)]
pub struct PoolGroup(u32);

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
    /// It is ground: it casts into the ground's own cascade, not the others.
    pub ground: bool,
}

/// A textured model to draw: the model, and its instance (placed and lit
/// as a mesh's: see `Instance`).
pub(crate) struct PbrDraw {
    pub model: crate::pbr::PbrModel,
    pub instance: Instance,
    /// It casts shadows; how far it reaches from its origin (metres, scaled).
    pub casts: bool,
    pub reach: f32,
    /// The part of it drawn (0: its body).
    pub part: u8,
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
    /// A globe's surface map (see `Frame::with_globe`): its layer + 1 (0:
    /// none), the world's kind (0 Earth-like, 1 dry, 2 cratered, 3 canonical terrain, 4 canonical terrain/water, 5 diagnostic categories), its relief
    /// (m, scaled), the brightness of its colours.
    pub globe: [f32; 4],
    /// Where its vertices are on the world, in radii: `pos * w + xyz` (a
    /// whole globe: (0, 0, 0, 1); a patch of ground with its own origin:
    /// that origin, and 1 / radius, its vertices in metres).
    pub globe_at: [f32; 4],
}

impl Instance {
    /// Its model scaled along its own axes by `axes` (each column of its turn and scale).
    pub(crate) fn scale_axes(&mut self, axes: Vec3) {
        for (c, k) in [(&mut self.c0, axes.x), (&mut self.c1, axes.y), (&mut self.c2, axes.z)] {
            c[0] *= k;
            c[1] *= k;
            c[2] *= k;
        }
    }
}
// (A patch's place for its fine grain rides in the columns' spare w: its
// origin (m, the world's own frame) wrapped to `MICRO_PERIOD`, its vertices
// added in the shader — exact to the millimetre, and the same across patches.
// A globe's air (see `Frame::with_air`) in `material`'s w (its depths, packed)
// and `t`'s (its shell, m).)

/// The fine grain on the ground repeats every this many metres (see `Instance::globe_micro`).
pub const MICRO_PERIOD: f64 = 4096.0;

/// A light source: a star. How bright it looks falls with the square of the
/// distance; the eye adapts only part of the way (`ADAPT`), so a planet
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

/// Lit faces shaded on the screen's colours (`colour × (ambient + diffuse max(0, n·l))`, under a world's sea
/// level its departure from 1 times `sea`), not lit in linear light: the planet lab's views. Exact with the tone
/// map off (`Graphics::tone_map`), where the screen is the light gamma-encoded.
/// A vertex's `data.w` saying its `data.xyz` is its direction on the planet cube (`Frame::planet_cube`).
pub const PLANET_CUBE_DATA: f32 = -1.0e6;

#[derive(Clone, Copy, Debug)]
pub struct ViewShade {
    pub ambient: f32,
    pub diffuse: f32,
    pub sea: f32,
    /// The world's centre (world) and its sea level's radius (m): below it is sea floor.
    pub centre: DVec3,
    pub radius: f64,
    /// How much steeper than they stand the normals given were made (land, sea: the package's light exaggeration;
    /// 1 and 1: as they stand). The shade takes them back down as a pixel's ground grows smaller: all of it at a
    /// pixel of `TAPER_FAR` m or more, none at `TAPER_NEAR` or less, log-linear between.
    pub land_light: f32,
    pub sea_light: f32,
}

/// A view-shade pixel's ground (m) at and over which a normal's exaggeration is kept whole, and at and under which
/// it is taken back to 1 (the planet lab's: x13 whole down to 2 km pixels, so 5,000 km up keeps the approved look;
/// 19.5 km left it at about x6.7 there, blurry, 2026-10-23).
pub const TAPER_FAR: f32 = 2_000.0;
pub const TAPER_NEAR: f32 = 100.0;

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
pub const ADAPT: f32 = 0.45;
/// The brightest light a face takes (several times the light at 1 AU from
/// a sun-like star: near a star, colours wash out to white).
pub const MAX_LIGHT: f32 = 4.0;

impl Light {
    /// Perceived brightness of its light at `p` (world), after adaptation.
    pub fn intensity_at(&self, p: DVec3) -> f32 {
        (self.irradiance_at(p) as f32).powf(ADAPT).clamp(0.02, MAX_LIGHT)
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

/// A glow's rim: twelve points around the unit circle (cos, sin).
const RIM: [(f32, f32); 12] = [
    (1.0, 0.0),
    (0.866_025_4, 0.5),
    (0.5, 0.866_025_4),
    (0.0, 1.0),
    (-0.5, 0.866_025_4),
    (-0.866_025_4, 0.5),
    (-1.0, 0.0),
    (-0.866_025_4, -0.5),
    (-0.5, -0.866_025_4),
    (0.0, -1.0),
    (0.5, -0.866_025_4),
    (0.866_025_4, -0.5),
];

/// Width and height of one character cell of the HUD font, in pixels.
pub const GLYPH: f32 = 8.0;

impl Frame {
    /// Submitted lines, triangles and points across the complete frame, including
    /// child views and repeated instances. Shadows/composition are not extra
    /// geometry submissions; PBR counts include only the requested part. These
    /// are demand counts, not GPU residency or post-culling raster counts.
    pub(crate) fn counts(&self) -> (u32, u32, u32) {
        let meshes = || self.meshes.iter().chain(&self.front).chain(&self.overlays);
        let mut lines = (self.lines.len() + self.front_lines.len() + self.hud.len()) / 2
            + meshes().filter(|m| m.edges).map(|m| m.mesh.edges.len()).sum::<usize>();
        let mut triangles = (self.solids.len() + self.hud_tris.len()) / 3
            + meshes().map(|m| m.mesh.faces.len()).sum::<usize>()
            + self.pbr.iter().map(|d| d.model.data.primitives.iter()
                .filter(|p| p.part == d.part).map(|p| p.indices.len() / 3).sum::<usize>())
                .sum::<usize>();
        let mut points = self.points.len() + self.sky.len();
        for (_, _, child, _) in &self.views {
            let (l, t, p) = child.counts();
            lines = lines.saturating_add(l as usize);
            triangles = triangles.saturating_add(t as usize);
            points = points.saturating_add(p as usize);
        }
        let count = |n: usize| n.min(u32::MAX as usize) as u32;
        (count(lines), count(triangles), count(points))
    }

    /// Retained scene into an isolated viewport. Rect uses this frame's HUD coordinates.
    /// Configure lighting/overlays on the child; its UI and depth are independent.
    /// View IDs must be unique per host frame; nested views are refused.
    pub fn scene_view(&mut self, id: u64, scene: &crate::scene::Scene,
        view: &crate::viewport::View, configure: impl FnOnce(&mut Frame)) {
        assert!(!self.views.iter().any(|(v,_,_,_)| *v == id), "duplicate view id");
        assert!(view.valid(), "invalid view");
        let r = view.rect;
        assert!(r.origin.is_finite() && r.size.is_finite() && r.origin.min_element() >= 0.
            && r.size.min_element() > 0. && (r.origin + r.size).cmple(self.size).all(), "invalid view rectangle");
        let mut child = Frame::new(view.camera, r.size, r.size);
        child.projection = view.projection;
        configure(&mut child);
        assert!(child.views.is_empty(), "nested views unsupported");
        scene.draw(&mut child, view.layers);
        self.views.push((id, r, Box::new(child), self.hud_tris.len()));
        self.canvas_scene();
    }

    pub(crate) fn new(camera: Camera, scene_size: Vec2, hud_size: Vec2) -> Self {
        Self {
            keepalive: Vec::new(),
            camera,
            projection: Default::default(),
            views: Vec::new(),
            clear: Color::BLACK,
            light: None,
            reflector: None,
            eclipsers: Vec::new(),
            shadow_reach: 0.0,
            graphics: Graphics::default(),
            state: DrawState::default(),
            scene_size,
            size: hud_size,
            studio: false,
            view_shade: None,
            sharpen: 0.0,
            planet_cube: None,
            sun_probe: None,
            shadow_focus: 0.0,
            ambient: SHADE_AMBIENT,
            sky: Vec::new(),
            solids: Vec::new(),
            lines: Vec::new(),
            front_lines: Vec::new(),
            points: Vec::new(),
            glows: Vec::new(),
            hud_tris: Vec::new(),
            canvas_scene: 1,
            hud: Vec::new(),
            meshes: Vec::new(),
            overlays: Vec::new(),
            mesh_uploads: Vec::new(),
            mesh_releases: Vec::new(),
            pool: Default::default(),
            pbr: Vec::new(),
            front: Vec::new(),
            front_glows: Vec::new(),
            globe_maps: Vec::new(),
            world_maps: None,
            world_turn: glam::DQuat::IDENTITY,
            world_clouds: [0.0; 4],
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
        if let crate::viewport::Projection::Orthographic { height, .. } = self.projection {
            return radius as f32 * self.scene_size.y / height;
        }
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
        self.with_state(|s| s.in_front = true, f);
    }

    /// Draw `f` with the state changed by `change`, then put it back as it was.
    fn with_state(&mut self, change: impl FnOnce(&mut DrawState), f: impl FnOnce(&mut Frame)) {
        let before = self.state;
        change(&mut self.state);
        f(self);
        self.state = before;
    }

    /// The opposite of `in_front`: what `f` draws is in the scene, hidden by
    /// what's nearer the eye, even while drawing in front.
    pub fn in_scene(&mut self, f: impl FnOnce(&mut Frame)) {
        self.with_state(|s| s.in_front = false, f);
    }

    /// A light's glow at `at` (world): a soft disc facing the eye (`RIM`), `radius`
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
        // (Under half a pixel, or wholly behind the eye: nothing to see.)
        let q = self.camera.orientation.as_dquat();
        if r < 0.5 * per_px || rel.dot(q * DVec3::NEG_Z) < -r {
            return;
        }
        // (Facing the eye: across its right and up.)
        let (right, up) = ((q * DVec3::X * r).as_vec3(), (q * DVec3::Y * r).as_vec3());
        let at = rel.as_vec3();
        let centre = Vertex { pos: at.to_array(), color: [light[0], light[1], light[2], 1.0] };
        let rim: [Vertex; 13] = std::array::from_fn(|k| {
            let (c, s) = RIM[k % 12];
            Vertex { pos: (at + right * c + up * s).to_array(), color: [0.0, 0.0, 0.0, 0.0] }
        });
        let glows = if self.state.in_front { &mut self.front_glows } else { &mut self.glows };
        for k in 0..12 {
            glows.extend([centre, rim[k], rim[k + 1]]);
        }
    }

    /// A triangle of added light (a glow's shape of your own: an
    /// atmosphere's rim), its colour at each corner (linear, may be over 1).
    pub fn glow_triangle(&mut self, p: [DVec3; 3], light: [[f32; 3]; 3]) {
        for k in 0..3 {
            let pos = self.rel(p[k]);
            (if self.state.in_front { &mut self.front_glows } else { &mut self.glows }).push(Vertex { pos, color: [light[k][0], light[k][1], light[k][2], 1.0] });
        }
    }

    /// Meshes drawn in `f` have this surface: `glint` (0 matte .. 1 polished
    /// metal), `sharp` (its power: 8 broad .. 80 tight), `glow` (lit by itself).
    pub fn with_surface(&mut self, glint: f32, sharp: f32, glow: f32, f: impl FnOnce(&mut Frame)) {
        self.with_state(|s| s.surface = [glint, sharp, glow, 0.0], f);
    }

    /// Meshes drawn in `f` are a world's globe surfaced from `map`: `kind`
    /// (0 Earth-like, 1 dry, 2 cratered) picks its palette, `relief` (m,
    /// as drawn) its heights for slope shading, `bright` its colours'
    /// brightness. Their fill tint is the world's own colour (and their
    /// vertex colours should be white).
    /// `at`: where its vertices are on the world (see `Instance::globe_at`).
    /// `micro`: a patch's origin (m, the world's frame) for its fine grain (none: zero).
    /// The world whose globe map is `globe` is drawn with its full-resolution `maps` (one
    /// world at a time: the one near the eye; the others with their globe maps alone).
    /// `center` and `radius`: where the world is and how big (m), for its air and sky.
    pub fn world_maps(&mut self, maps: &std::sync::Arc<crate::worldmaps::WorldMaps>, globe: &std::sync::Arc<crate::model::GlobeMap>, center: glam::DVec3, radius: f64) {
        self.world_maps = Some((maps.clone(), globe.id(), center, radius));
    }

    /// The bound world's turn now (its own frame to the scene's) and its clouds: the year's phase
    /// in its months (0..12), El Niño's index, world seconds (wrapped by the caller).
    pub fn world_clouds(&mut self, turn: glam::DQuat, month: f32, enso: f32, time_s: f32) {
        self.world_turn = turn;
        self.world_clouds = [month, enso, time_s, 1.0];
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_globe(&mut self, map: &std::sync::Arc<crate::model::GlobeMap>, kind: f32, relief: f32, bright: f32, at: [f32; 4], micro: glam::DVec3, f: impl FnOnce(&mut Frame)) {
        let k = match self.globe_maps.iter().position(|m| m.id() == map.id()) {
            Some(k) => k,
            None => {
                self.globe_maps.push(map.clone());
                self.globe_maps.len() - 1
            }
        };
        let wrap = micro.map(|v| v.rem_euclid(MICRO_PERIOD)).as_vec3();
        self.with_state(
            |s| {
                s.globe = [k as f32 + 1.0, kind, relief, bright];
                s.globe_at = at;
                s.globe_micro = [wrap.x, wrap.y, wrap.z, 0.0];
            },
            f,
        );
    }

    /// Meshes drawn in `f` (a globe, its ground) are seen through air: `depth`
    /// the optical depth of its column straight up, in red, green and blue
    /// (0..2.55), `shell` how thick a layer of even density it's drawn as (m).
    /// Seen through it, the ground fades toward the sunlit air's own light.
    pub fn with_air(&mut self, depth: [f32; 3], shell: f32, f: impl FnOnce(&mut Frame)) {
        // (Packed in 24 bits, a hundredth each, exact in an f32: see `air_depth` in the shader.)
        let q = depth.map(|d| (d * 100.0).round().clamp(0.0, 255.0));
        self.with_state(|s| s.globe_air = [q[0] * 65536.0 + q[1] * 256.0 + q[2], shell], f);
    }

    /// Meshes drawn in `f` cast no shadows (a planet's globe: its night is
    /// its own shading, and its eclipses are `eclipsers`).
    pub fn no_shadow(&mut self, f: impl FnOnce(&mut Frame)) {
        self.with_state(|s| s.casts = false, f);
    }

    /// Meshes drawn in `f` are ground: they cast only into the ground's own
    /// shadow cascade (kilometres across, coarse texels), so a mountain
    /// shades the valley beyond it without the ground shading itself up
    /// close in the fine cascades.
    pub fn ground_shadow(&mut self, f: impl FnOnce(&mut Frame)) {
        self.with_state(
            |s| {
                s.casts = false;
                s.ground = true;
            },
            f,
        );
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
        let clip = self.projection.matrix(&self.camera, self.size.x / self.size.y) * self.camera.relative(p).extend(1.0);
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
        let (r, light) = (self.reflector.filter(|_| self.graphics.planet_light)?, self.light?);
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
        // (Its colour half washed out: ground of many colours, and air, between.)
        let grey = (r.color[0] + r.color[1] + r.color[2]) / 3.0;
        (base * s > 0.001).then(|| ((-up).as_vec3(), s, (base, [0, 1, 2].map(|j| (0.5 * r.color[j] + 0.5 * grey) * light.color[j] * k))))
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
        let inst = self.instance(t, line, fill, lit);
        let d = MeshDraw { mesh: mesh.clone(), instance: inst, edges, casts: self.state.casts, reach: mesh.radius() * t.scale as f32, ground: self.state.ground };
        if self.state.in_front { self.front.push(d) } else { self.meshes.push(d) }
    }

    /// Unlit transparent mesh, scaled along its local axes. Shares GPU geometry across
    /// instances, blends far to near after opaque terrain/models, tests depth and never writes it.
    pub fn model_overlay_scaled(&mut self, mesh: &Mesh, t: &Transform, axes: Vec3, color: Color) {
        let mut instance = self.instance(t, [0.; 4], color.0, false);
        instance.scale_axes(axes);
        self.overlays.push(MeshDraw { mesh: mesh.clone(), instance, edges: false, casts: false,
            reach: mesh.radius() * axes.abs().max_element() * t.scale.abs() as f32, ground: false });
    }

    /// A group of pooled draws (see `pooled`): every member turned and scaled by `t` and lit as at
    /// its position, edges in `line` at `edges` of their brightness (0: none), faces in `fill`;
    /// each colour times the mesh's own (a face's own colour if it has one, else its vertices').
    /// Unlit: full brightness. (Pooled meshes cast no shadows yet.)
    pub fn pool_group(&mut self, t: &Transform, line: Color, fill: Color, edges: f32, lit: bool) -> PoolGroup {
        let [r, g, b, a] = line.0;
        let inst = self.instance(t, [r * edges, g * edges, b * edges, a], fill.0, lit);
        self.pool.groups.push((inst, edges > 0.0));
        PoolGroup(self.pool.groups.len() as u32 - 1)
    }

    /// Draw pooled `mesh` at `origin` in `group`'s look (copied into the pool first if it isn't:
    /// it shows from the frame it's in).
    pub fn pooled(&mut self, group: PoolGroup, mesh: &crate::pool::PooledMesh, origin: DVec3) {
        let d = crate::pool::Draw { t: self.rel(origin), group: group.0, mesh: self.pool.mesh(mesh), morph: -1.0 };
        if self.state.in_front { self.pool.front.push(d) } else { self.pool.draws.push(d) }
    }

    /// Pooled geometry with one explicit morph weight, shared by every vertex of the tile.
    /// The offset stored in each vertex still supplies its exact parent-surface target.
    pub fn pooled_morphed(&mut self, group: PoolGroup, mesh: &crate::pool::PooledMesh, origin: DVec3, weight: f32) {
        self.pooled(group, mesh, origin);
        let draws = if self.state.in_front { &mut self.pool.front } else { &mut self.pool.draws };
        draws.last_mut().unwrap().morph = weight.clamp(0.0, 1.0);
    }

    /// Take on what `old`, a frame replaced before it was drawn, asked of the GPU besides drawing: its
    /// uploads and releases (a release is sent once; lost, its range would stay till pool pressure).
    pub(crate) fn carry_requests(&mut self, old: &Frame) {
        self.mesh_uploads.extend(old.mesh_uploads.iter().cloned());
        self.mesh_releases.extend_from_slice(&old.mesh_releases);
        self.pool.uploads.extend(old.pool.uploads.iter().cloned());
        self.pool.releases.extend_from_slice(&old.pool.releases);
    }

    /// Evict streamed pooled geometry once no draw in this frame names it.
    pub fn release_pooled(&mut self, id: u64) { self.pool.releases.push(id); }

    /// Protect an existing GPU allocation needed for a pending cover, without uploading it.
    /// Unprotected allocations can be evicted under pool pressure while CPU meshes stay cached.
    pub fn retain_pooled(&mut self, id: u64) { self.pool.keep.push(id); }

    /// Copy `mesh` into the pool without drawing it (`PooledMesh::gpu_resident` says when it is).
    pub fn prepare_pooled(&mut self, mesh: &crate::pool::PooledMesh) {
        self.pool.uploads.push(mesh.clone());
    }

    /// Stage an immutable mesh without drawing it. Poll Mesh::gpu_resident before group publication.
    pub fn prepare_mesh(&mut self, mesh: &Mesh) { self.mesh_uploads.push(mesh.clone()); }
    /// Its owner is done with mesh `id`: the renderer drops its buffers now rather than after
    /// they go unused a while (unless drawn this frame). A reservation still waits for the GPU.
    pub fn release_mesh(&mut self, id: u64) { self.mesh_releases.push(id); }

    /// Keep a CPU asset owner alive until this frame has been consumed.
    /// For PBR preparation spanning frames, also use `PbrModel::with_lease`.
    pub fn keep_alive(&mut self, owner: std::sync::Arc<dyn Send + Sync>) {
        self.keepalive.push(owner);
    }

    /// Draw a textured model, lit by the sun, shadows and reflected light.
    pub fn model_pbr(&mut self, model: &crate::pbr::PbrModel, t: &Transform) {
        self.model_pbr_part(model, t, 0);
    }

    /// One part of such a model (see `PbrModel::load_gltf_parts`; 0: its body), placed by `t`.
    pub fn model_pbr_part(&mut self, model: &crate::pbr::PbrModel, t: &Transform, part: u8) {
        let inst = self.instance(t, [1.0; 4], [1.0; 4], true);
        self.pbr.push(PbrDraw { model: model.clone(), instance: inst, casts: self.state.casts, reach: model.radius() * t.scale as f32, part });
    }

    /// As `model_pbr_part`, the part stretched along the model's own axes by `axes` (on top of `t.scale`)
    /// first: one model drawing every size of a family. (Its normals turn with the stretch, not against it:
    /// near enough for stretches of a few times.)
    pub fn model_pbr_scaled(&mut self, model: &crate::pbr::PbrModel, t: &Transform, axes: Vec3, part: u8) {
        let mut inst = self.instance(t, [1.0; 4], [1.0; 4], true);
        inst.scale_axes(axes);
        let reach = model.radius() * t.scale as f32 * axes.max_element();
        self.pbr.push(PbrDraw { model: model.clone(), instance: inst, casts: self.state.casts, reach, part });
    }

    /// Where a draw is (camera-relative), its tints, and the light on it here.
    fn instance(&self, t: &Transform, line: [f32; 4], fill: [f32; 4], lit: bool) -> Instance {
        let origin = t.position - self.camera.position;
        let m = glam::Mat3::from_quat(t.rotation) * t.scale as f32;
        let at = origin.as_vec3();
        let mut inst = Instance {
            c0: m.x_axis.extend(0.0).to_array(),
            c1: m.y_axis.extend(0.0).to_array(),
            c2: m.z_axis.extend(0.0).to_array(),
            t: at.extend(self.state.globe_air[1]).to_array(),
            line_tint: line,
            fill_tint: fill,
            // Unlit: full brightness (ambient 1).
            light_dir: [0.0, 0.0, 0.0, 1.0],
            light_color: [0.0, 0.0, 0.0, 1.0],
            refl_dir: [0.0; 4],
            refl_color: [0.0; 4],
            material: self.state.surface,
            globe: self.state.globe,
            globe_at: self.state.globe_at,
        };
        (inst.c0[3], inst.c1[3], inst.c2[3]) = (self.state.globe_micro[0], self.state.globe_micro[1], self.state.globe_micro[2]);
        inst.material[3] = self.state.globe_air[0];
        if lit && let Some(light) = self.light {
            let dir = (light.position - t.position).normalize_or_zero().as_vec3();
            // (What a planet or moon leaves of the sun here.)
            let seen = self.sun_visible(t.position) as f32;
            let c = self.light_at(at).map(|c| c * seen);
            inst.light_dir = dir.extend(self.ambient).to_array();
            inst.light_color = [c[0], c[1], c[2], LINE_AMBIENT];
            if let Some((d, s, (base, c))) = self.fill_at(t.position) {
                inst.refl_dir = d.extend(s).to_array();
                inst.refl_color = [c[0], c[1], c[2], base];
            }
        }
        inst
    }

    /// Line with a color at each end (blended along it).
    pub fn line2(&mut self, a: DVec3, b: DVec3, ca: Color, cb: Color) {
        let (a, b) = (self.rel(a), self.rel(b));
        let lines = if self.state.in_front { &mut self.front_lines } else { &mut self.lines };
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
            self.hud_tris.push(HudVertex { pos: [p[k].x, p[k].y], uv: solid, color: c[k].0, depth: [0.; 2], alpha: [0.; 2], scene: 0 });
        }
    }

    /// Begin an independent 3D canvas (e.g. a catalogue card). Opaque/masked
    /// geometry precedes globally depth-sorted blends. UI submitted after the
    /// first geometry is an overlay over this whole scene; start another canvas
    /// for a new camera/card. UI never writes canvas depth.
    pub fn canvas_scene(&mut self) {
        self.canvas_scene += 1;
    }

    /// Perspective triangle in layout pixels, with original eye distances.
    /// Reversed depth and perspective-correct colour interpolation run on GPU.
    pub fn canvas_triangle(&mut self, p: [Vec2; 3], distance: [f32; 3], near: f32, c: [Color; 3]) {
        let alpha = if c.iter().any(|c| c.0[3] < 1.) { CanvasAlpha::Blend } else { CanvasAlpha::Opaque };
        self.canvas_triangle_alpha(p, distance, near, c, alpha);
    }

    /// Explicit material mode. Opaque/mask output alpha one; blends preserve
    /// alpha and depth-test without writing depth. Mask discards below cutoff.
    pub fn canvas_triangle_alpha(&mut self, p: [Vec2; 3], distance: [f32; 3], near: f32, c: [Color; 3], alpha: CanvasAlpha) {
        assert!(near.is_finite() && near > 0. && distance.iter().all(|z| z.is_finite() && *z >= near));
        let alpha = match alpha {
            CanvasAlpha::Opaque => [0., 0.],
            CanvasAlpha::Mask(cutoff) => {
                assert!(cutoff.is_finite() && cutoff >= 0.);
                [1., cutoff]
            }
            CanvasAlpha::Blend => [2., 0.],
        };
        for k in 0..3 {
            self.hud_tris.push(HudVertex { pos: p[k].to_array(), uv: [0.; 2], color: c[k].0,
                depth: [distance[k], near], alpha, scene: self.canvas_scene });
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
                self.hud_tris.push(HudVertex { pos: [v.x, v.y], uv: crate::font::atlas().solid, color: c.0, depth: [0.; 2], alpha: [0.; 2], scene: 0 });
            }
        }
    }

    /// Filled rectangle in HUD pixel coordinates.
    pub fn hud_rect(&mut self, pos: Vec2, size: Vec2, color: Color) {
        let (a, b) = (pos, pos + size);
        for [x, y] in [[a.x, a.y], [b.x, a.y], [b.x, b.y], [a.x, a.y], [b.x, b.y], [a.x, b.y]] {
            self.hud_tris.push(HudVertex { pos: [x, y], uv: crate::font::atlas().solid, color: color.0, depth: [0.; 2], alpha: [0.; 2], scene: 0 });
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

    /// Draw with `f` inside the HUD rectangle `lo`..`hi` only: its lines cut
    /// at the edges, its fills and text kept only where wholly inside.
    pub fn hud_clipped(&mut self, lo: Vec2, hi: Vec2, f: impl FnOnce(&mut Frame)) {
        let (lines, tris) = (self.hud.len(), self.hud_tris.len());
        f(self);
        let inside = |p: Vec2| p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y;
        // Lines: each pair cut to the box (Liang-Barsky), or gone.
        let added: Vec<Vertex> = self.hud.drain(lines..).collect();
        for pair in added.as_chunks::<2>().0 {
            let (a, b) = (Vec2::new(pair[0].pos[0], pair[0].pos[1]), Vec2::new(pair[1].pos[0], pair[1].pos[1]));
            let d = b - a;
            let (mut t0, mut t1) = (0.0f32, 1.0f32);
            let mut keep = true;
            for (p, q) in [(-d.x, a.x - lo.x), (d.x, hi.x - a.x), (-d.y, a.y - lo.y), (d.y, hi.y - a.y)] {
                if p == 0.0 {
                    if q < 0.0 {
                        keep = false;
                    }
                } else {
                    let r = q / p;
                    if p < 0.0 {
                        t0 = t0.max(r);
                    } else {
                        t1 = t1.min(r);
                    }
                }
            }
            if keep && t0 <= t1 {
                let (pa, pb) = (a + d * t0, a + d * t1);
                self.hud.push(Vertex { pos: [pa.x, pa.y, 0.0], color: pair[0].color });
                self.hud.push(Vertex { pos: [pb.x, pb.y, 0.0], color: pair[1].color });
            }
        }
        // Fills and glyphs: each triangle wholly inside, or gone.
        let added: Vec<HudVertex> = self.hud_tris.drain(tris..).collect();
        for t in added.as_chunks::<3>().0 {
            if t.iter().all(|v| inside(Vec2::new(v.pos[0], v.pos[1]))) {
                self.hud_tris.extend_from_slice(t);
            }
        }
    }

    /// Ellipse outline in HUD pixel coordinates.
    /// A HUD line where it falls, not snapped to whole units (curves and
    /// slants: snapped, their pieces would step and wobble).
    pub fn hud_line_smooth(&mut self, a: Vec2, b: Vec2, color: Color) {
        self.hud.push(Vertex { pos: [a.x, a.y, 0.0], color: color.0 });
        self.hud.push(Vertex { pos: [b.x, b.y, 0.0], color: color.0 });
    }

    pub fn hud_ellipse(&mut self, center: Vec2, radii: Vec2, segments: u32, color: Color) {
        let at = |i: u32| {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            center + Vec2::new(a.cos(), a.sin()) * radii
        };
        for i in 0..segments {
            self.hud_line_smooth(at(i), at(i + 1), color);
        }
    }

    /// Draw text (the HUD typeface, on its monospaced grid); `\n` starts a
    /// new line. Returns the end position.
    /// A label anchored in world space. Depth-tested labels use the view's actual
    /// scene depth (opaque/masked geometry); glass does not occlude them.
    pub fn world_text(&mut self, at: DVec3, text: &str, color: Color, scale: f32, depth_tested: bool) {
        let clip = self.projection.matrix(&self.camera, self.size.x/self.size.y) * self.camera.relative(at).extend(1.);
        if clip.w <= 0. || !(0.0..=1.0).contains(&(clip.z/clip.w)) { return; }
        let pos = Vec2::new((clip.x/clip.w+1.)*0.5*self.size.x, (1.-clip.y/clip.w)*0.5*self.size.y);
        let start = self.hud_tris.len();
        self.text_scaled(pos, text, color, scale);
        if depth_tested {
            for vertex in &mut self.hud_tris[start..] { vertex.alpha = [3., clip.z/clip.w]; }
        }
    }

    pub fn text(&mut self, pos: Vec2, text: &str, color: Color) -> Vec2 {
        self.text_scaled(pos, text, color, 1.0)
    }

    /// `text`, its glyphs `scale` times the usual size (small labels).
    pub fn text_scaled(&mut self, pos: Vec2, text: &str, color: Color, scale: f32) -> Vec2 {
        let atlas = crate::font::atlas();
        let origin = pos;
        let mut cursor = origin;
        // (Each glyph centred in its cell of the grid.)
        let inset = (GLYPH - atlas.advance) * 0.5;
        for ch in text.chars() {
            if ch == '\n' {
                cursor = Vec2::new(origin.x, cursor.y + (GLYPH + 2.0) * scale);
                continue;
            }
            let g = crate::font::glyph_index(ch).and_then(|k| atlas.glyphs.get(k).copied()).unwrap_or(atlas.glyphs[(b'?' - 32) as usize]);
            if g.at[2] > 0.0 {
                let p = cursor + Vec2::new(inset + g.at[0], crate::font::BASELINE + g.at[1]) * scale;
                let (a, b) = (p, p + Vec2::new(g.at[2], g.at[3]) * scale);
                let (u0, v0, u1, v1) = (g.uv[0], g.uv[1], g.uv[2], g.uv[3]);
                let v = |x: f32, y: f32, u: f32, w: f32| HudVertex { pos: [x, y], uv: [u, w], color: color.0, depth: [0.; 2], alpha: [0.; 2], scene: 0 };
                self.hud_tris.extend([v(a.x, a.y, u0, v0), v(b.x, a.y, u1, v0), v(b.x, b.y, u1, v1), v(a.x, a.y, u0, v0), v(b.x, b.y, u1, v1), v(a.x, b.y, u0, v1)]);
            }
            cursor.x += GLYPH * scale;
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

#[cfg(test)]
mod count_tests {
    use super::*;
    #[test]
    fn complete_frame_counts_instances_parts_front_and_views_once() {
        use crate::pbr::{PbrData, PbrModel, Primitive};
        let model = PbrModel::new(PbrData {
            primitives: vec![
                Primitive { vertices: vec![], indices: vec![0; 6], material: 0, part: 0 },
                Primitive { vertices: vec![], indices: vec![0; 9], material: 0, part: 1 },
            ], ..Default::default()
        });
        let mut frame = Frame::new(Camera::default(), Vec2::splat(100.), Vec2::splat(100.));
        let t = Transform::default();
        frame.model_pbr(&model, &t); // two body triangles, not the other part
        frame.model_pbr(&model, &t); // a second instance counts again
        frame.in_front(|f| f.line(DVec3::ZERO, DVec3::X, Color::WHITE));
        let mesh = Mesh::new(crate::model::WireModel {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            edges: vec![[0, 1]], faces: vec![[0, 1, 2]], ..Default::default()
        });
        frame.in_front(|f| f.model(&mesh, &t, Color::WHITE, Color::WHITE));
        let mut child = Frame::new(Camera::default(), Vec2::splat(50.), Vec2::splat(50.));
        child.model_pbr_part(&model, &t, 1);
        child.point(DVec3::ZERO, Color::WHITE);
        child.hud_rect(Vec2::ZERO, Vec2::ONE, Color::WHITE);
        frame.views.push((1, crate::viewport::PixelRect { origin: Vec2::ZERO, size: Vec2::splat(50.) }, Box::new(child), 0));
        assert_eq!(frame.counts(), (2, 10, 1));
    }
}

#[cfg(test)]
mod pyramid_tests {
    use super::*;
    /// A frame replaced before it was drawn hands its uploads and releases to the next.
    #[test]
    fn replaced_frame_requests_carry_over() {
        let mesh=crate::PooledMesh::new(&crate::PoolModel {positions:vec![Vec3::ZERO,Vec3::X,Vec3::Y],faces:vec![[0,1,2]],..Default::default()});
        let mut old=Frame::new(Camera::default(),Vec2::splat(100.),Vec2::splat(100.));
        old.prepare_pooled(&mesh);old.release_pooled(7);old.release_mesh(9);
        let mut new=Frame::new(Camera::default(),Vec2::splat(100.),Vec2::splat(100.));
        new.release_pooled(8);
        new.carry_requests(&old);
        assert_eq!(new.pool.releases,vec![8,7]);assert_eq!(new.mesh_releases,vec![9]);assert_eq!(new.pool.uploads.len(),1);
    }
    #[test]
    fn pyramid_morph_override_preserves_distance_mode_and_explicit_eviction() {
        let mesh=crate::PooledMesh::new(&crate::PoolModel {positions:vec![Vec3::ZERO,Vec3::X,Vec3::Y],faces:vec![[0,1,2]],morph:vec![crate::Morph {offset:Vec3::Z,near:1.,far:2.};3],..Default::default()});
        let mut f=Frame::new(Camera::default(),Vec2::splat(100.),Vec2::splat(100.));
        let g=f.pool_group(&Transform::default(),Color::WHITE,Color::WHITE,0.,false);
        f.pooled(g,&mesh,DVec3::ZERO);f.pooled_morphed(g,&mesh,DVec3::ZERO,0.75);f.release_pooled(mesh.id());
        assert_eq!(f.pool.draws[0].morph,-1.);assert_eq!(f.pool.draws[1].morph,0.75);assert_eq!(f.pool.releases,vec![mesh.id()]);
        // (As scene.wgsl lays out PoolDraw: a vec3, six words, rounded up to its 16-byte alignment.)
        assert_eq!(std::mem::size_of::<crate::pool::GpuDraw>(),48);
    }
}
