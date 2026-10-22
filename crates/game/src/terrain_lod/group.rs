//! The optional contact group at runtime (`UNIVERSE_PGS1_CONTACT_GROUP`): the shared worker built
//! off the draw thread, its meshes reserved as a whole (16 MiB, active, uploading and retired
//! together), staged until every one is on the GPU, then drawn in place of the resident patches
//! it replaces, all at once. Any input patch remade, split, merged or gone and the group is
//! dropped, the resident ground drawn as before. Canonical physics is never touched.
use super::contact::{ResidentPatch, Snapshot};
use super::{Key, Patch};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use universe_engine::glam::{DVec3, Vec3};
use universe_engine::mesh_budget::{MeshBudget, mesh_bytes};
use universe_engine::{Frame, Mesh, WireModel};
use universe_sim::Body;
use universe_terrain_contact::worker;

/// Additional GPU bytes for contact groups at most: active, uploading and retired together.
pub(crate) const GPU_BUDGET: usize = 16 << 20;
/// A new group is asked for when the eye comes within this of its core's edge (m).
const REANCHOR_MARGIN: f64 = 8.;
/// Frames the ground under a group must stay changed before a new one is built for it.
const CHANGE_HOLD: u64 = 30;
/// Frames to wait after a refusal before asking again.
const COOLDOWN: u64 = 60;

/// What a group is and what it depends on.
pub(crate) struct Meta {
    pub id: u64,
    pub source_generation: u64,
    pub centre: DVec3,
    pub east: DVec3,
    pub north: DVec3,
    pub radius: f64,
    pub band_origin: DVec3,
    /// Every resident patch it was built from (for the record).
    pub inputs: Vec<(Key, u64)>,
    /// The patches it replaces, which cover its whole footprint: each must still be drawn, the
    /// same mesh. The draw selection never overlaps itself, so while they are, nothing else can
    /// reach the footprint; a patch beyond them changing meets them as any neighbour does.
    pub replaced: Vec<(Key, u64)>,
    pub report: Value,
}
impl Meta {
    fn chart(&self, dir: DVec3) -> Option<[f64; 2]> {
        let den = dir.dot(self.centre);
        (den > 0.).then(|| [self.radius * dir.dot(self.east) / den, self.radius * dir.dot(self.north) / den])
    }
    pub fn in_core(&self, dir: DVec3, margin: f64) -> bool {
        self.chart(dir).is_some_and(|p| p.iter().all(|x| x.abs() <= worker::CORE_HALF - margin))
    }
    fn current(&self, shown: &HashSet<Key>, patches: &HashMap<Key, Patch>) -> bool {
        self.replaced.iter().all(|(k, id)| shown.contains(k) && patches.get(k).is_some_and(|p| p.mesh.id() == *id))
    }
    fn resident(&self, patches: &HashMap<Key, Patch>) -> bool {
        self.replaced.iter().all(|(k, id)| patches.get(k).is_some_and(|p| p.mesh.id() == *id))
    }
}

/// A built group's CPU models, before any GPU reservation.
pub(crate) struct Built {
    meta: Meta,
    /// Replaced patches: their models (None: nothing of them is drawn) and ground triangle counts.
    patches: Vec<(Key, Option<WireModel>, usize)>,
    band: WireModel,
    rim: WireModel,
}

/// A group's meshes, reserved; staged until all are resident, then drawn.
pub(crate) struct Published {
    pub meta: Arc<Meta>,
    pub patches: HashMap<Key, Option<(Mesh, usize)>>,
    pub band: Mesh,
    pub rim: Option<Mesh>,
    bytes: usize,
}
impl Published {
    fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        self.patches.values().flatten().map(|(m, _)| m).chain(std::iter::once(&self.band)).chain(self.rim.iter())
    }
}

type Slot = Arc<Mutex<Option<Result<Built, Value>>>>;

pub(crate) struct Runtime {
    budget: MeshBudget,
    /// Bumped when the source changes: running jobs refuse, groups go.
    source: Arc<AtomicU64>,
    job: Option<Slot>,
    pub staged: Option<Published>,
    pub active: Option<Published>,
    next_id: u64,
    cooldown: u64,
    pub events: Vec<Value>,
    /// Retired meshes' ids, told to the renderer every frame until this frame (a frame may be
    /// replaced before it is drawn); their reservations then wait only for the GPU.
    releasing: Vec<(u64, u64)>,
    /// Since when the ground under the newest group, as the eye would now have it, differs.
    changed_since: Option<u64>,
}
impl Default for Runtime {
    fn default() -> Self {
        Self { budget: MeshBudget::new(GPU_BUDGET), source: Arc::new(AtomicU64::new(1)), job: None, staged: None, active: None, next_id: 1, cooldown: 0, events: vec![], releasing: vec![], changed_since: None }
    }
}

impl Runtime {
    pub fn source_generation(&self) -> u64 {
        self.source.load(Ordering::Acquire)
    }
    fn event(&mut self, frame: u64, what: &str, detail: Value) {
        if self.events.len() < 10_000 {
            self.events.push(json!({"frame":frame,"event":what,"detail":detail}));
        }
    }
    fn release(&mut self, group: Published, frame: u64) {
        self.releasing.extend(group.meshes().map(|m| (m.id(), frame + 600)));
    }
    /// A new source: running jobs refuse, every group goes.
    pub fn invalidate(&mut self, frame: u64) {
        self.source.fetch_add(1, Ordering::AcqRel);
        self.job = None;
        for group in [self.staged.take(), self.active.take()].into_iter().flatten() {
            self.event(frame, "retired", json!({"group":group.meta.id,"why":"source changed"}));
            self.release(group, frame);
        }
    }
    /// The patches the active group replaces: the selector keeps them as they are (neither split
    /// nor merged) while a group stands on them, so it holds until the eye nears its core's edge.
    pub fn pinned(&self) -> HashSet<Key> {
        // (Only the active group's: a staged one may stand on coarser or finer ground, and is
        // only kept resident until it takes over.)
        self.active.iter().flat_map(|p| p.meta.replaced.iter().map(|(k, _)| *k)).collect()
    }
    /// Bytes: (active, uploading, retired but not yet released by the GPU).
    pub fn gpu_bytes(&self) -> (usize, usize, usize) {
        let active = self.active.as_ref().map_or(0, |p| p.bytes);
        let staged = self.staged.as_ref().map_or(0, |p| p.bytes);
        (active, staged, self.budget.live_bytes().saturating_sub(active + staged))
    }

    /// Once a frame, after the draw selection `shown`: drop what no longer holds, take in a
    /// finished job, and start one when the eye needs a group and none is building.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        frame: u64,
        shown: &[Key],
        natural: &HashSet<Key>,
        settled: bool,
        patches: &HashMap<Key, Patch>,
        snapshot: Option<Arc<Snapshot>>,
        eye: DVec3,
        sys: &Arc<universe_sim::StarSystem>,
        body: usize,
        pool: &rayon::ThreadPool,
    ) {
        let shown: HashSet<Key> = shown.iter().copied().collect();
        for which in [true, false] {
            // The active group's patches are drawn; a staged one's still resident, unchanged.
            let slot = if which { &mut self.active } else { &mut self.staged };
            if slot.as_ref().is_some_and(|p| if which { !p.meta.current(&shown, patches) } else { !p.meta.resident(patches) }) {
                let group = slot.take().expect("checked");
                let witness: Vec<Value> = group.meta.replaced.iter().filter(|(k, id)| !(shown.contains(k) || !which) || patches.get(k).is_none_or(|p| p.mesh.id() != *id)).take(3)
                    .map(|(k, id)| json!({"key":[k.face,k.level,k.x,k.y],"resident":patches.get(k).map(|p| p.mesh.id() == *id),"shown":shown.contains(k),"pinned_descendant_shown":shown.iter().any(|s| s.level > k.level && s.face == k.face && (s.x >> (s.level - k.level), s.y >> (s.level - k.level)) == (k.x, k.y)),"ancestor_shown":std::iter::successors(k.parent(), |p| p.parent()).find(|a| shown.contains(a)).map(|a| json!({"level":a.level,"missing_below":std::iter::successors(Some(*k), |p| p.parent()).take_while(|p| p.level > a.level).filter_map(|p| p.parent()).flat_map(|q| q.children()).filter(|c| !patches.contains_key(c)).map(|c| json!([c.level,c.x,c.y])).collect::<Vec<_>>()}))})).collect();
                self.event(frame, "retired", json!({"group":group.meta.id,"why":"a replaced patch is no longer drawn as it was","stage":if which {"active"} else {"staged"},"witness":witness}));
                self.release(group, frame);
            }
        }
        let finished = self.job.as_ref().and_then(|slot| slot.lock().ok().and_then(|mut s| s.take()));
        if let Some(result) = finished {
            self.job = None;
            match result {
                Ok(built) if built.meta.source_generation == self.source_generation() && built.meta.current(natural, patches) => {
                    let mut models = vec![built.band, built.rim];
                    let mut keys = vec![];
                    for (k, m, ground) in built.patches {
                        keys.push((k, ground, m.is_some()));
                        models.extend(m);
                    }
                    let bytes: usize = models.iter().filter_map(mesh_bytes).sum();
                    match self.budget.group(models) {
                        Ok(meshes) => {
                            let mut meshes = meshes.into_iter();
                            let band = meshes.next().expect("band");
                            let rim = meshes.next().filter(|m| !m.faces.is_empty());
                            let patches = keys.into_iter().map(|(k, ground, has)| (k, has.then(|| (meshes.next().expect("patch"), ground)))).collect();
                            let mut meta = built.meta;
                            meta.id = self.next_id;
                            self.next_id += 1;
                            self.event(frame, "staged", json!({"group":meta.id,"bytes":bytes,"report":meta.report}));
                            if let Some(old) = self.staged.replace(Published { meta: Arc::new(meta), patches, band, rim, bytes }) {
                                self.event(frame, "retired", json!({"group":old.meta.id,"why":"a newer group staged","stage":"staged"}));
                                self.release(old, frame);
                            }
                        }
                        Err(_) => {
                            self.cooldown = frame + COOLDOWN;
                            self.event(frame, "refused", json!({"why":"GPU budget","bytes":bytes,"live":self.budget.live_bytes()}));
                        }
                    }
                }
                Ok(_) => self.event(frame, "refused", json!({"why":"stale on arrival"})),
                Err(e) => {
                    self.cooldown = frame + COOLDOWN;
                    self.event(frame, "refused", e);
                }
            }
        }
        // A group is wanted: none yet, the eye near its core's edge, or the ground under it as the
        // eye would now have it changed. Built only from ground settled round the eye.
        let dir = eye.normalize();
        let newest = self.staged.as_ref().or(self.active.as_ref());
        // (The ground changing under it must hold a while first: from a height, or moving fast,
        // the selection there flickers between levels, and each rebuild costs a job and an upload.)
        let changed = newest.is_some_and(|p| !p.meta.current(natural, patches));
        self.changed_since = if changed { self.changed_since.or(Some(frame)) } else { None };
        let changed = self.changed_since.is_some_and(|since| frame - since >= CHANGE_HOLD);
        let wanted = newest.is_none_or(|p| !p.meta.in_core(dir, REANCHOR_MARGIN)) || changed;
        if !wanted || !settled || self.job.is_some() || frame < self.cooldown {
            return;
        }
        let Some(snapshot) = snapshot.filter(|s| s.complete()) else { return };
        let input = match snapshot.worker_input() {
            Ok(input) => input,
            Err(e) => {
                self.cooldown = frame + COOLDOWN;
                self.event(frame, "refused", json!(e));
                return;
            }
        };
        let slot: Slot = Arc::default();
        let (out, source, sys) = (slot.clone(), self.source.clone(), sys.clone());
        let generation = source.load(Ordering::Acquire);
        self.job = Some(slot);
        self.event(frame, "job", json!({"eye":snapshot.eye.to_array(),"generation":generation}));
        pool.spawn(move || {
            let _scope = universe_prof::scope("assets/terrain/contact group");
            let s = input.snapshot.surface.clone().expect("worker input has its source");
            let result = worker::group(&s, input.snapshot.radius, input.snapshot.eye.to_array(), generation, &source, &input.selected, &input.patches)
                .map(|g| built(g, &input.snapshot, &sys.bodies[body], generation));
            if let Ok(mut o) = out.lock() {
                *o = Some(result);
            }
        });
    }

    /// Before the frame's selection: a staged group all on the GPU, its patches still resident
    /// as built, replaces the active one (whose meshes go).
    pub fn promote(&mut self, frame: u64, patches: &HashMap<Key, Patch>) {
        let ready = self.staged.as_ref().is_some_and(|p| p.meta.resident(patches) && p.meshes().all(|m| m.gpu_resident()));
        if !ready {
            return;
        }
        let staged = self.staged.take();
        let old = std::mem::replace(&mut self.active, staged);
        let id = self.active.as_ref().map(|p| p.meta.id);
        self.event(frame, "published", json!({"group":id,"replaces":old.as_ref().map(|p| p.meta.id)}));
        if let Some(old) = old {
            self.release(old, frame);
        }
    }

    /// Before drawing: stage what is uploading; tell the renderer what retired.
    pub fn prepare(&mut self, frame: &mut Frame, frame_no: u64) {
        self.releasing.retain(|&(id, until)| {
            frame.release_mesh(id);
            frame_no < until
        });
        if let Some(staged) = self.staged.as_ref() {
            for m in staged.meshes().filter(|m| !m.gpu_resident()) {
                frame.prepare_mesh(m);
            }
        }
    }
}

/// The worker's group as models to draw: replaced patches keep their resident attributes
/// (blended where new points are), the band reads its own at each point.
fn built(g: worker::Group, snapshot: &Snapshot, body: &Body, source_generation: u64) -> Built {
    let by_key: HashMap<[u32; 4], &ResidentPatch> =
        snapshot.patches.iter().map(|p| ([p.key.face as u32, p.key.level as u32, p.key.x, p.key.y], p)).collect();
    let blend = |p: &ResidentPatch, b: &[(u32, f32); 3]| -> [f32; 4] {
        let mut out = [0f32; 4];
        for (i, w) in b {
            let d = p.mesh.data.get(*i as usize).copied().unwrap_or([0.; 4]);
            for k in 0..4 {
                out[k] += d[k] * w;
            }
        }
        out
    };
    let model = |positions: &[[f32; 3]], faces: Vec<[u32; 3]>, colors: Vec<[f32; 4]>, data: Vec<[f32; 4]>| WireModel {
        positions: positions.iter().map(|p| Vec3::from_array(*p)).collect(),
        faces,
        colors,
        data,
        smooth: true,
        ..WireModel::default()
    };
    let patches = g
        .patches
        .iter()
        .map(|r| {
            let p = by_key[&r.key];
            let m = (!r.faces.is_empty()).then(|| {
                let data = r.blend.iter().map(|b| blend(p, b)).collect();
                model(&r.positions, r.faces.clone(), r.morph.clone(), data)
            });
            (p.key, m, r.ground)
        })
        .collect();
    let band_origin = DVec3::from_array(g.band.origin);
    // Wound to face out: counter-clockwise on the chart (east, north) faces its centre's way.
    let band_faces = g
        .band
        .triangles
        .iter()
        .map(|t| {
            let q = t.map(|i| g.band.uv[i as usize]);
            let s = (q[1][0] - q[0][0]) * (q[2][1] - q[0][1]) - (q[1][1] - q[0][1]) * (q[2][0] - q[0][0]);
            if s < 0. { [t[0], t[2], t[1]] } else { *t }
        })
        .collect();
    let band_data = g
        .band
        .vertices
        .iter()
        .map(|v| super::fields_at(body, g.radius, (band_origin + Vec3::from_array(*v).as_dvec3()).normalize()).0)
        .collect();
    let band = model(&g.band.vertices, band_faces, vec![[0., 0., 0., 128.]; g.band.vertices.len()], band_data);
    let rim_data = g
        .rim
        .blend
        .iter()
        .map(|(slot, b)| by_key.get(&g.inputs[*slot as usize].0).map_or([0.; 4], |p| blend(p, b)))
        .collect();
    let rim = model(&g.rim.positions, g.rim.faces.clone(), g.rim.morph.clone(), rim_data);
    let key_of = |k: [u32; 4]| by_key[&k].key;
    Built {
        meta: Meta {
            id: 0,
            source_generation,
            centre: DVec3::from_array(g.centre),
            east: DVec3::from_array(g.east),
            north: DVec3::from_array(g.north),
            radius: g.radius,
            band_origin,
            inputs: g.inputs.iter().map(|(k, id)| (key_of(*k), *id)).collect(),
            replaced: g.patches.iter().map(|r| (key_of(r.key), r.mesh_id)).collect(),
            report: g.report,
        },
        patches,
        band,
        rim,
    }
}

/// The readiness trace (`UNIVERSE_PGS1_READINESS_TRACE` = its path): one record a frame in the
/// planets lab's `planet-readiness-trace/1`, the subject the camera, the ground under it measured
/// from what was submitted that frame. Frames appended every 120 to `<trace>.frames.jsonl`; written whole when dropped.
pub(crate) struct Trace {
    path: std::path::PathBuf,
    sha: String,
    radius: f64,
    frames: Vec<Value>,
    written: usize,
    /// Wall clock at the trace's start: each frame's `wall_s` times the frames as shown.
    start: std::time::Instant,
}
impl Trace {
    /// Bound to the source the world actually holds: the SHA256 of the bytes it was read from
    /// (none: no trace).
    pub fn from_env(radius: f64, surface: &universe_sim::world::worlds::pgs::Surface) -> Option<Self> {
        let path: std::path::PathBuf = crate::devenv::var_os("UNIVERSE_PGS1_READINESS_TRACE")?.into();
        let Some(sha) = crate::pgs_preview::loaded_hash(surface) else {
            log::warn!("readiness trace: the world's source is not the one this process loaded and hashed; no trace");
            return None;
        };
        let _ = std::fs::remove_file(path.with_extension("frames.jsonl"));
        Some(Self { path, sha: sha.to_owned(), radius, frames: vec![], written: 0, start: std::time::Instant::now() })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn record(&mut self, frame: u64, time_s: f64, on_foot: bool, eye: DVec3, surface: &universe_sim::world::worlds::pgs::Surface, submitted: Option<f64>, runtime: &Runtime, pending: usize, resident: usize) {
        if self.frames.len() >= 36_000 {
            return;
        }
        let dir = eye.normalize();
        // Each frame bound to the source the world holds now (empty: not the one loaded).
        let sha = crate::pgs_preview::loaded_hash(surface).unwrap_or("");
        let (bed, water) = match surface.query(dir) {
            Ok(s) => (json!(s.height_m), match s.water {
                universe_sim::world::worlds::pgs::Water::Dry => "dry",
                universe_sim::world::worlds::pgs::Water::Wet { .. } => "wet",
                universe_sim::world::worlds::pgs::Water::Unknown => "unknown",
            }),
            Err(_) => (Value::Null, "unknown"),
        };
        let (active, uploading, retired) = runtime.gpu_bytes();
        let group = runtime.active.as_ref().map(|p| {
            json!({"id":format!("g{}",p.meta.id),"source_sha256":sha,"source_generation":p.meta.source_generation,
                "centre":p.meta.centre.to_array(),"east":p.meta.east.to_array(),"north":p.meta.north.to_array(),"core_half_width_m":worker::CORE_HALF})
        });
        let ready = runtime.active.as_ref().is_some_and(|p| p.meta.in_core(dir, 0.));
        self.frames.push(json!({
            "frame":frame,"simulation_time_s":time_s,"wall_s":self.start.elapsed().as_secs_f64(),"subject":if on_foot {"walker"} else {"camera"},"source_sha256":sha,"source_generation":runtime.source_generation(),
            "foot_direction":dir.to_array(),"ready":ready,"water":water,"canonical_bed_height_m":bed,
            "submitted_ground_height_m":submitted.map(|h| h - self.radius),
            "gpu_bytes":{"active":active,"uploading":uploading,"retired":retired},
            "pending":pending,"resident_patches":resident,"workers":2,"active_group":group,
        }));
        // (Appended in batches as lines: rewriting the whole trace here cost a frame 35-50 ms.)
        if self.frames.len() - self.written >= 120 {
            self.append();
        }
    }
    /// The frames not yet appended, one JSON line each, to `<trace>.frames.jsonl` (kept if the run
    /// dies before `write`).
    fn append(&mut self) {
        use std::io::Write;
        let lines: String = self.frames[self.written..].iter().map(|f| f.to_string() + "\n").collect();
        let file = std::fs::OpenOptions::new().create(true).append(true).open(self.path.with_extension("frames.jsonl"));
        if let Ok(mut file) = file {
            let _ = file.write_all(lines.as_bytes());
        }
        self.written = self.frames.len();
    }
    pub fn write(&mut self, runtime: &Runtime) {
        // A walker only if every frame's subject was one (the eye over the walker's feet).
        let walker = !self.frames.is_empty() && self.frames.iter().all(|f| f["subject"] == "walker");
        let root = json!({"format":"planet-readiness-trace/1","source_sha256":self.sha,"host_radius_m":self.radius,
            "measurement":"submitted_geometry","subject":if walker {"walker"} else {"camera"},"frames":self.frames});
        let tmp = self.path.with_extension("tmp");
        if std::fs::write(&tmp, serde_json::to_vec(&root).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
        let events: String = runtime.events.iter().map(|e| e.to_string() + "\n").collect();
        let _ = std::fs::write(self.path.with_extension("events.jsonl"), events);
        self.append();
    }
}

/// Where a ray from `eye` straight down meets `mesh` (drawn at `origin`, body frame), as the
/// shader places its vertices for that eye: the distance from the world's centre, highest hit.
pub(crate) fn ray_hit(mesh: &Mesh, ground: usize, origin: DVec3, eye: DVec3) -> Option<f64> {
    let d = -eye.normalize();
    let start = eye - origin;
    let vertex = |id: u32| {
        let own = mesh.positions[id as usize];
        let c = mesh.colors.get(id as usize).copied().unwrap_or([0.; 4]);
        universe_terrain_contact::boundary_correction::retained::morphed(origin, own, c, eye).as_dvec3()
    };
    let mut best: Option<f64> = None;
    for f in mesh.faces.iter().take(ground) {
        let [a, b, c] = f.map(vertex);
        let (e1, e2) = (b - a, c - a);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-12 {
            continue;
        }
        let tv = start - a;
        let u = tv.dot(p) / det;
        let q = tv.cross(e1);
        let v = d.dot(q) / det;
        if u < -1e-9 || v < -1e-9 || u + v > 1. + 1e-9 {
            continue;
        }
        let along = e2.dot(q) / det;
        let at = (eye + d * along).length();
        best = Some(best.map_or(at, |h: f64| h.max(at)));
    }
    best
}
