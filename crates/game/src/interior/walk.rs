//! Isolated walking session, using the shared world walker.
use super::*;

/// The test stand: a design with no hull stood on the ground (its landing legs'
/// feet) and walked, first person, in the studio. What's solid: its rooms' walls and
/// floors, its modules, the ground; a ramp a slope from the ground up to the
/// airlock it serves; an airlock a passage (floor, walls, roof, open at its ends);
/// a vertical room a shaft climbed as a ladder. The walk is the game's own walker.
pub(super) struct Stand {
    native_mesh: Option<Arc<universe_sim::world::walk::WalkMesh>>,
    hatches: Vec<StandHatch>,
    native_model: Option<(String, universe_engine::glam::DVec3)>,
    scene: std::cell::RefCell<universe_engine::scene::Scene>,
    clock: universe_sim::clocks::Runtime,
    jump_pending: f64,
    pub(super) mesh: universe_sim::world::walk::WalkMesh,
    /// What's drawn: each face and its colour (the first two the ground, beneath
    /// all); the frame's members as lines; the rooms (axis ends, half width,
    /// half height), to know when the walker's inside.
    pub(super) faces: Vec<([Vec3; 3], Color)>,
    pub(super) equipment: Vec<crate::equipment_visual::Placed>,
    pub(super) rooms: Vec<(Vec3, Vec3, f32, f32)>,
    /// Airlock interiors use the module's full orientation, including roll.
    airlock_rooms: Vec<(Vec3, glam::Quat, Vec3)>,
    /// Each module's name on each face of its box: the face's middle, which way it
    /// faces, how wide it is (m), the name.
    pub(super) labels: Vec<(Vec3, Vec3, f32, String)>,
    pub(super) members: Vec<[Vec3; 2]>,
    /// The shafts climbed: each one's axis and radius.
    pub(super) shafts: Vec<(Vec3, Vec3, f32)>,
    pub(super) feet: universe_engine::glam::DVec3,
    pub(super) velocity: universe_engine::glam::DVec3,
    pub(super) yaw: f64,
    pub(super) pitch: f64,
}

/// Geometry is compiled once in the document's frame. Rendering and the shared
/// walker translate the same leaf by the same proximity function; neither uses
/// the native model's authored-origin offset.
struct StandHatch {
    id: universe_engine::scene::ObjectId,
    leaf: Leaf,
    visual: universe_engine::model::Mesh,
    collision: universe_sim::world::walk::WalkMesh,
}

impl StandHatch {
    fn new(id: universe_engine::scene::ObjectId, leaf: Leaf, visual: universe_engine::model::Mesh) -> Self {
        // Match models::hatches' 8 cm slab, using the actual convex leaf outline.
        // Do not fill its enclosing box: rounded/chamfered doorway corners stay
        // clear. The inset window is glass, so remains solid with the leaf.
        let mut triangles = Vec::new();
        let outline = &leaf.outline;
        if outline.len() >= 3 {
            let normal = (outline[1] - outline[0]).cross(outline[2] - outline[0]).normalize_or_zero();
            for side in [-0.04, 0.04] {
                for k in 1..outline.len() - 1 {
                    triangles.push([outline[0], outline[k], outline[k + 1]]
                        .map(|p| (p + normal * side).as_dvec3()));
                }
            }
            for k in 0..outline.len() {
                let (a, b) = (outline[k], outline[(k + 1) % outline.len()]);
                let q = [a - normal * 0.04, b - normal * 0.04, b + normal * 0.04, a + normal * 0.04];
                triangles.push([q[0], q[1], q[2]].map(|p| p.as_dvec3()));
                triangles.push([q[0], q[2], q[3]].map(|p| p.as_dvec3()));
            }
        }
        Self { id, leaf, visual, collision: universe_sim::world::walk::WalkMesh::new(&triangles) }
    }

    fn shift(&self, feet: universe_engine::glam::DVec3) -> universe_engine::glam::DVec3 {
        let eye = (feet + universe_engine::glam::DVec3::Y * universe_sim::world::crew::EYE).as_vec3();
        let open = ((2.8 - eye.distance(self.leaf.middle)) / 1.2).clamp(0., 1.);
        let open = open * open * (3. - 2. * open);
        (self.leaf.open * open).as_dvec3()
    }

    fn set_node(&self, scene: &mut universe_engine::scene::Scene, feet: universe_engine::glam::DVec3) {
        use universe_engine::scene::{Geometry, Node};
        let position = self.shift(feet);
        let mut node = if let Some(node) = scene.get(self.id) {
            if node.pose.position == position { return; }
            node.clone()
        } else {
            Node { geometry: Some(Geometry::Mesh {
                mesh: self.visual.clone(), line: Color([0.8, 0.8, 0.8, 1.]), fill: Color([1.; 4]),
            }), ..Default::default() }
        };
        node.pose.position = position;
        scene.set(self.id, node).expect("valid local hatch pose");
    }
}

/// A box's faces (`lo` to `hi`), less those `skip` says (0..6: -y, +y, -z, +z,
/// -x, +x).
fn box_faces(lo: Vec3, hi: Vec3, skip: &[usize]) -> Vec<[Vec3; 3]> {
    let c = |x: f32, y: f32, z: f32| Vec3::new(if x < 0.5 { lo.x } else { hi.x }, if y < 0.5 { lo.y } else { hi.y }, if z < 0.5 { lo.z } else { hi.z });
    let quads = [
        [c(0., 0., 0.), c(1., 0., 0.), c(1., 0., 1.), c(0., 0., 1.)],
        [c(0., 1., 0.), c(0., 1., 1.), c(1., 1., 1.), c(1., 1., 0.)],
        [c(0., 0., 0.), c(0., 1., 0.), c(1., 1., 0.), c(1., 0., 0.)],
        [c(0., 0., 1.), c(1., 0., 1.), c(1., 1., 1.), c(0., 1., 1.)],
        [c(0., 0., 0.), c(0., 0., 1.), c(0., 1., 1.), c(0., 1., 0.)],
        [c(1., 0., 0.), c(1., 1., 0.), c(1., 1., 1.), c(1., 0., 1.)],
    ];
    quads.iter().enumerate().filter(|(k, _)| !skip.contains(k)).flat_map(|(_, q)| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
}

/// A ramp's inferred floor/ground endpoints in its oriented local envelope.
/// Moving along local up preserves pitch/roll; a near-vertical local up cannot
/// reach a horizontal floor, so retain the authored top/bottom in that case.
fn ramp_ends(b: &Block, lock: Option<&Block>, ground: f32) -> (Vec3, Vec3, Vec3) {
    let rotation = b.orientation();
    let along_z = b.size.z >= b.size.x;
    let axis = if along_z { Vec3::Z } else { Vec3::X };
    let sign = lock.map(|l| (rotation.inverse() * (l.at - b.at)).dot(axis))
        .filter(|d| d.abs() > 1e-6).map_or(1., f32::signum);
    let along = axis * if along_z { b.size.z * 0.5 } else { b.size.x * 0.5 } * sign;
    let up = rotation * Vec3::Y;
    let mut near = b.place(along + Vec3::Y * b.size.y * 0.5);
    let mut far = b.place(-along - Vec3::Y * b.size.y * 0.5);
    if up.y.abs() > 1e-3 {
        let top = lock.map_or(near.y, |l| l.place(Vec3::NEG_Y * l.size.y * 0.5).y);
        near += up * ((top - near.y) / up.y);
        far += up * ((ground - far.y) / up.y);
    }
    let half = rotation * if along_z { Vec3::X * b.size.x * 0.5 } else { Vec3::Z * b.size.z * 0.5 };
    (near, far, half)
}

/// Imported walk vertices and authored stand geometry already share the same
/// centred design frame. Keep movement and floor picking on this one collider set.
fn stand_colliders<'a>(mesh: &'a universe_sim::world::walk::WalkMesh, native: Option<&'a universe_sim::world::walk::WalkMesh>, hatches: &'a [StandHatch], feet: universe_engine::glam::DVec3) -> Vec<universe_sim::world::walk::Collider<'a>> {
    use universe_engine::glam::{DQuat, DVec3};
    use universe_sim::world::walk::Collider;
    std::iter::once(mesh).chain(native).map(|mesh| Collider::Mesh {
        mesh, at: DVec3::ZERO, rot: DQuat::IDENTITY,
    }).chain(hatches.iter().map(|hatch| Collider::Mesh {
        mesh: &hatch.collision, at: hatch.shift(feet), rot: DQuat::IDENTITY,
    })).collect()
}

impl Interior {
    /// The design stood on its test stand, ready to walk from the foot of its
    /// boarding ramp (or, with none, on the ground before it).
    pub(super) fn stand_up(&self) -> Stand {
        use universe_engine::glam::DVec3;
        // Resolve the same fitted default as placed_module before constructing
        // labels, fallbacks and collision envelopes. Never modify the document.
        let blocks: Vec<_> = self.plan.blocks.iter().map(|b| {
            let mut b = b.clone();
            let fit = self.fit.iter().find(|f| f.id == kind(&b.id));
            b.rotation = Some(b.rotation.unwrap_or_else(|| crate::equipment_visual::rotation(
                fit.and_then(|f| push_of(&b, f).map(|p| p.0))
            )));
            b
        }).collect();
        let kind_of = |b: &Block| self.fit.iter().find(|f| f.id == kind(&b.id)).and_then(figures).map(|(k, ..)| k).unwrap_or_default();
        let ground = blocks.iter().filter(|b| kind_of(b) == "landing_gear").map(|b| b.bounds().0.y).fold(f32::INFINITY, f32::min);
        let ground = if ground.is_finite() { ground } else { blocks.iter().map(|b| b.bounds().0.y).fold(0.0, f32::min) };
        let native_mesh = self.spec().and_then(|s| s.shape().walk.clone());
        // A native hull can extend below the document origin or fitted modules.
        // Keep the test ground below it, rather than cutting through its decks.
        let ground = native_mesh.as_ref().filter(|m| m.lo.y.is_finite())
            .map_or(ground, |m| ground.min(m.lo.y as f32));
        // The ground: wide, flat, at the legs' feet (drawn first: beneath all).
        let g = 300.0;
        let ground_colour = Color([0.24, 0.27, 0.22, 1.0]);
        let mut faces: Vec<([Vec3; 3], Color)> = vec![
            ([Vec3::new(-g, ground, -g), Vec3::new(g, ground, -g), Vec3::new(g, ground, g)], ground_colour),
            ([Vec3::new(-g, ground, -g), Vec3::new(g, ground, g), Vec3::new(-g, ground, g)], ground_colour),
        ];
        // Its rooms' walls and floors.
        for p in self.panels().iter() {
            let q = &p.outline;
            let c = p.colour();
            for k in 1..q.len().saturating_sub(1) {
                faces.push(([q[0], q[k], q[k + 1]], Color(c)));
            }
        }
        // Its decks: floors (but one under a walkway: its floor is drawn with it).
        let under_walkway = |plate: &Plate| {
            let m = Vec3::new((plate.lo.x + plate.hi.x) * 0.5, plate.y, (plate.lo.y + plate.hi.y) * 0.5);
            self.plan.lines.iter().enumerate().any(|(k, &PathLine(a, b, pr, _))| {
                let (pa, pb) = (self.plan.points[a].at, self.plan.points[b].at);
                let d = pb - pa;
                let t = (m - pa).dot(d) / d.length_squared().max(1e-6);
                let off = m - (pa + d * t.clamp(0.0, 1.0));
                pr.stand && self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled) && (0.0..=1.0).contains(&t) && off.y.abs() < 0.3 && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5
            })
        };
        for plate in self.plan.plates.iter().filter(|p| !under_walkway(p)) {
            let c = [Vec3::new(plate.lo.x, plate.y, plate.lo.y), Vec3::new(plate.hi.x, plate.y, plate.lo.y), Vec3::new(plate.hi.x, plate.y, plate.hi.y), Vec3::new(plate.lo.x, plate.y, plate.hi.y)];
            let colour = Color([0.6, 0.55, 0.45, 1.0]);
            faces.push(([c[0], c[1], c[2]], colour));
            faces.push(([c[0], c[2], c[3]], colour));
        }
        let authored_decks=self.decks_now.as_ref().map(|p|deck_geometry(p,self.spec().and_then(|s|s.shape().walk.as_deref())));
        if let Some(b)=&authored_decks {
            for (q,floor) in &b.panels {let c=if *floor {DECK}else{Color([0.62,0.65,0.68,1.])};let q=q.map(|p|p.as_vec3());faces.push(([q[0],q[1],q[2]],c));faces.push(([q[0],q[2],q[3]],c));}
        }
        let structure_faces = faces.len();
        // Its modules: solid, but an airlock a passage and a ramp a slope; each one's
        // name on every face of its box, to know what's walked past.
        let airlocks: Vec<&Block> = blocks.iter().filter(|b| kind_of(b) == "airlock").collect();
        let mut start = None;
        let mut labels = Vec::new();
        let mut equipment = Vec::with_capacity(blocks.len());
        for b in &blocks {
            let fit = self.fit.iter().find(|f| f.id == kind(&b.id));
            if let Some(f) = fit {
                let h = b.size * 0.5;
                for n in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
                    // (Its width across: a side's along the ground, the top's longer way.)
                    let wide = if n.y != 0.0 { b.size.x.max(b.size.z) } else if n.x != 0.0 { b.size.z } else { b.size.x };
                    labels.push((b.place(n * (h.dot(n.abs()) + 0.02)), b.orientation() * n, wide, f.name.clone()));
                }
            }
            let first = faces.len();
            let k = kind_of(b);
            let along_z = b.size.z >= b.size.x;
            match k.as_str() {
                "airlock" => {
                    // (Open at its ends, along its length.)
                    let open = if along_z { [2, 3] } else { [4, 5] };
                    for t in box_faces(-b.size * 0.5, b.size * 0.5, &open) {
                        faces.push((t.map(|p| b.place(p)), Color([0.62, 0.66, 0.7, 1.0])));
                    }
                }
                "ramp" => {
                    // (From the end nearest an airlock, at that airlock's floor, down
                    // to the ground at the far end.)
                    let lock = airlocks.iter().min_by(|a, c| a.at.distance(b.at).total_cmp(&c.at.distance(b.at)));
                    let (near, far, half) = ramp_ends(b, lock.copied(), ground);
                    let (a, c, d, e) = (near - half, near + half, far + half, far - half);
                    let colour = Color([0.85, 0.7, 0.35, 1.0]);
                    faces.push(([a, c, d], colour));
                    faces.push(([a, d, e], colour));
                    // (Walked up from its foot, facing up it.)
                    let up = near - far;
                    start = Some((far + (far - near).normalize_or_zero() * 1.5, f64::from((-up.x).atan2(-up.z))));
                }
                _ => {
                    let round = self.round(&b.id);
                    let colour = if k == "landing_gear" { Color([0.4, 0.42, 0.46, 1.0]) } else { Color([0.55, 0.5, 0.62, 1.0]) };
                    for (t, _) in b.faces(round) {
                        faces.push((t, colour));
                    }
                }
            }
            // A pending airlock/ramp must show the same open passage/slope
            // that is walked, rather than an unrelated closed-box fallback.
            equipment.push(crate::equipment_visual::placed_rotated(
                fit.map_or("", |f| f.key.as_str()), b.at, b.orientation(),
                || faces[first..].iter().map(|(triangle, _)| *triangle).collect(),
            ));
        }
        let tris: Vec<[DVec3; 3]> = faces.iter().map(|(t, _)| t.map(|p| p.as_dvec3())).collect();
        let mesh = universe_sim::world::walk::WalkMesh::new(&tris);
        faces.truncate(structure_faces); // drawing is resolved once; collision stays above
        // The shafts: walled rooms standing upright.
        let mut shafts: Vec<_> = (0..self.plan.lines.len()).filter(|&k| self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled)).filter_map(|k| {
            let (a, b) = self.plan.axis(k);
            ((b - a).normalize_or_zero().y.abs() > 0.9).then_some((a.min(b), a.max(b), self.plan.lines[k].2.width * 0.5))
        }).collect();
        if let Some(b)=&authored_decks {shafts.extend(b.climbs.iter().map(|(lo,hi)|{let lo=lo.as_vec3();let hi=hi.as_vec3();let x=(lo.x+hi.x)*0.5;let z=(lo.z+hi.z)*0.5;(Vec3::new(x,lo.y,z),Vec3::new(x,hi.y,z),(hi.x-lo.x)*0.5)}));}
        let (feet, yaw) = start.unwrap_or_else(|| (Vec3::new(0.0, ground, -40.0), 0.0));
        let mut scene = universe_engine::scene::Scene::default();
        let mut geometry = universe_engine::model::WireModel::default();
        for (triangle, colour) in &faces {
            let first = geometry.positions.len() as u32;
            geometry.positions.extend(triangle);
            geometry.colors.extend([[colour.0[0],colour.0[1],colour.0[2],1.]; 3]);
            geometry.faces.push([first, first + 1, first + 2]);
        }
        scene.set(universe_engine::scene::ObjectId(0), universe_engine::scene::Node {
            geometry: Some(universe_engine::scene::Geometry::Mesh {
                mesh: universe_engine::model::Mesh::new(geometry),
                line: Color([0.; 4]), fill: Color([1.; 4]),
            }), ..Default::default()
        }).expect("valid compiled structure");
        let feet = DVec3::new(f64::from(feet.x), f64::from(ground) + 0.05, f64::from(feet.z));
        let leaves = self.plan.leaves();
        // Call the existing cached geometry builder only at session construction.
        let hatch_meshes = crate::models::hatches(&leaves);
        let hatches: Vec<_> = leaves.into_iter().zip(hatch_meshes).enumerate().map(|(index, (leaf, mesh))| {
            // 0 is structure, 1..=equipment.len() are modules, MAX is native hull.
            StandHatch::new(universe_engine::scene::ObjectId((equipment.len() + index + 1) as u64), leaf, mesh)
        }).collect();
        for hatch in &hatches { hatch.set_node(&mut scene, feet); }
        Stand {
            // import.rs subtracts made_centre when constructing shape.walk.
            // Only the original authored model below still needs that offset.
            native_mesh,
            hatches,
            native_model: self.spec().and_then(|s|s.visual.as_ref().map(|v|(v.clone(),s.shape().made_centre))),
            scene: std::cell::RefCell::new(scene),
            clock: universe_sim::clocks::Runtime::new(&universe_sim::clocks::Subscriptions::of(&[universe_sim::world::registry::ClockKey::Realtime]), 1.0, universe_sim::universe::TICK_BUDGET as u32).expect("registry realtime clock"),
            jump_pending: 0.,
            equipment,
            mesh,
            faces,
            labels,
            members: self.plan.beams.iter().map(|b| [b.a, b.b]).collect(),
            rooms: (0..self.plan.lines.len()).filter(|&k| self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled)).map(|k| {
                let (a, b) = self.plan.axis(k);
                let p = self.plan.lines[k].2;
                (a, b, p.width * 0.5, p.height * 0.5)
            }).collect(),
            airlock_rooms: airlocks.iter().map(|b| (b.at, b.orientation(), b.size * 0.5)).collect(),
            shafts,
            feet,
            velocity: DVec3::ZERO,
            yaw,
            pitch: 0.0,
        }
    }
}

/// Where a walker clicked at would stand on the stand: the floor of the first room
/// the ray from `eye` along `dir` passes into; else the first floor it meets (a
/// side met: what's under that spot, backed off it a little); none if it meets
/// nothing.
pub(super) fn stand_spot(s: &Stand, eye: Vec3, dir: Vec3) -> Option<universe_engine::glam::DVec3> {
    use universe_engine::glam::DVec3;
    let d = dir.as_dvec3().try_normalize()?;
    if !eye.is_finite() { return None; }
    let colliders = stand_colliders(&s.mesh, s.native_mesh.as_deref(), &s.hatches, s.feet);
    let mut from = eye.as_dvec3();
    // (Along the ray, surface after surface: the first room's floor it passes over,
    // or else the first floor it meets; a side met, what's under that spot.)
    let mut first: Option<DVec3> = None;
    for _ in 0..16 {
        let Some((t, n)) = universe_sim::world::walk::ray(&colliders, from, d, 2000.0) else { break };
        let hit = from + d * t;
        let level = n.y.abs() > 0.6;
        let spot = if level && d.y < 0.0 {
            Some(hit + DVec3::Y * 0.05)
        } else if !level {
            let back = hit - d * 0.4;
            universe_sim::world::walk::ray(&colliders, back, DVec3::NEG_Y, 2000.0).map(|(down, _)| back - DVec3::Y * (down - 0.05))
        } else {
            None
        };
        if let Some(p) = spot {
            if in_rooms(s, (p + DVec3::Y * 0.9).as_vec3()) {
                return Some(p);
            }
            first = first.or(Some(p));
        }
        from = hit + d * 0.05;
    }
    first
}

/// Is `p` inside one of the stand's rooms (or airlocks)?
fn in_rooms(s: &Stand, p: Vec3) -> bool {
    s.airlock_rooms.iter().any(|&(at, rotation, half)| {
        (rotation.inverse() * (p - at)).abs().cmple(half).all()
    }) || s.rooms.iter().any(|&(a, b, hw, hh)| {
        let d = b - a;
        let t = ((p - a).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let off = p - (a + d * t);
        if d.normalize_or_zero().y.abs() > 0.9 { Vec2::new(off.x, off.z).length() < hw } else { off.y.abs() < hh && Vec3::new(off.x, 0.0, off.z).length() < hw }
    })
}

/// The test stand walked for a frame: WASD, the arrows or a drag to look, SPACE to
/// jump, SHIFT to run (up a shaft: forward climbs, looking down goes down); ESC
/// back to the studio.
pub(super) fn stand_input(ctx: &Context, s: &mut Stand, g: f64) {
    use universe_engine::glam::DVec3;
    use universe_sim::world::walk::{Stride, Walker};
    let input = &ctx.input;
    let dt = f64::from(ctx.dt);
    let key = |k: KeyCode| if input.down(k) { 1.0 } else { 0.0 };
    let turn = key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft);
    let tilt = key(KeyCode::ArrowUp) - key(KeyCode::ArrowDown);
    s.yaw -= turn * 1.8 * dt;
    s.pitch = (s.pitch + tilt * 1.2 * dt).clamp(-1.4, 1.4);
    if input.button_down(MouseButton::Left) {
        s.yaw -= f64::from(input.mouse_delta.x) * 0.006;
        s.pitch = (s.pitch - f64::from(input.mouse_delta.y) * 0.006).clamp(-1.4, 1.4);
    }
    let forward = key(KeyCode::KeyW) - key(KeyCode::KeyS);
    let right = key(KeyCode::KeyD) - key(KeyCode::KeyA);
    let run = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let (sn, co) = s.yaw.sin_cos();
    let (fwd, side) = (DVec3::new(-sn, 0.0, -co), DVec3::new(co, 0.0, -sn));
    let speed = if run { universe_sim::world::crew::RUN } else { universe_sim::world::crew::WALK };
    let wish = (fwd * forward + side * right).clamp_length_max(1.0) * speed;
    let jump = if input.pressed(KeyCode::Space) { universe_sim::world::crew::JUMP } else { 0.0 };
    let climb = forward * universe_sim::world::crew::CLIMB * if s.pitch < -0.5 { -1.0 } else { 1.0 };
    // (A shaft's ladder runs a metre past its top, so the climber's feet come level
    // with the floor there to step off.)
    let shafts = &s.shafts;
    let climbable = |p: DVec3| shafts.iter().any(|&(lo, hi, r)| {
        let q = p.as_vec3();
        q.y >= lo.y - 0.3 && q.y <= hi.y + 1.0 && Vec2::new(q.x - lo.x, q.z - lo.z).length() < r
    });
    let mut colliders = stand_colliders(&s.mesh, s.native_mesh.as_deref(), &s.hatches, s.feet);
    let hatch_start = colliders.len() - s.hatches.len();
    let hatches = &s.hatches;
    let mut w = Walker { feet: s.feet, velocity: s.velocity };
    s.jump_pending = s.jump_pending.max(jump);
    let jump_pending = &mut s.jump_pending;
    s.clock.advance(dt, &mut |_, _, step| {
        // The shared walker sees the moving leaves each fixed step. Reuse each
        // compiled WalkMesh and the collider list; only translations change.
        for (collider, hatch) in colliders[hatch_start..].iter_mut().zip(hatches) {
            if let universe_sim::world::walk::Collider::Mesh { at, .. } = collider {
                *at = hatch.shift(w.feet);
            }
        }
        w.step(&colliders, &|_| DVec3::Y, g, &climbable, &Stride { wish, jump: *jump_pending, climb }, step);
        *jump_pending = 0.;
    });
    s.feet = w.feet;
    s.velocity = w.velocity;
}

/// All geometry uses the same retained renderer as flight; only annotations are UI.
pub(super) fn draw_stand(frame: &mut Frame, s: &Stand, place: &str) {
    use universe_engine::{scene::ObjectId, viewport::{View, Projection, PixelRect}};
    let _scope = universe_prof::scope("draw/studio/walk");
    let size = frame.size();
    let eye = s.feet + universe_engine::glam::DVec3::Y * universe_sim::world::crew::EYE;
    let (sy, cy) = (s.yaw as f32).sin_cos();
    let (sp, cp) = (s.pitch as f32).sin_cos();
    let forward = Vec3::new(-sy * cp, sp, -cy * cp);
    let mut camera = universe_engine::Camera {position: eye, fov_y: FOV, near: 0.08, ..Default::default()};
    camera.look_at(eye + forward.as_dvec3(), Vec3::Y);
    let view = View {camera, projection: Projection::Perspective,
        rect: PixelRect {origin: Vec2::ZERO, size}, layers: u64::MAX};
    let mut scene = s.scene.borrow_mut();
    for hatch in &s.hatches { hatch.set_node(&mut scene, s.feet); }
    for (index, visual) in s.equipment.iter().enumerate() {
        visual.set_node(&mut scene, &ObjectId(index as u64 + 1), None).expect("valid compiled equipment");
    }
    // Stand's native path/offset are immutable. Once installed, this node's
    // leased model is the retained result; do not request or upsert every frame.
    if let Some((path, centre)) = &s.native_model {
        if scene.get(ObjectId(u64::MAX)).is_none() {
            if let Some(visual) = crate::visual_assets::request_native(std::path::Path::new(path), &[])
                .filter(|v| v.neutral_error.is_none()) {
                scene.set(ObjectId(u64::MAX), universe_engine::scene::Node {
                    pose: universe_engine::scene::Pose {position: -*centre,..Default::default()},
                    geometry:Some(universe_engine::scene::Geometry::Model {model:visual.model.with_lease(visual.clone()),part:0}),
                    ..Default::default()
                }).expect("valid native hull pose");
            }
        }
    }
    frame.scene_view(1, &scene, &view, |child| {
        child.studio = true;
        child.ambient = 0.45;
        child.clear = Color([0.02, 0.03, 0.06, 1.]);
        if !in_rooms(s, eye.as_vec3()) {
            for [a,b] in &s.members {child.line(a.as_dvec3(), b.as_dvec3(), Color([0.4,0.95,0.55,0.9]));}
        }
        let focal = size.y * 0.5 / (FOV * 0.5).tan();
        for (at, normal, width, name) in &s.labels {
            let delta = eye.as_vec3() - *at;
            if normal.dot(delta) <= 0. || delta.length() > 40. { continue; }
            let depth = -delta.dot(forward);
            if depth < camera.near + 0.2 {continue;}
            let fit = 0.9 * width * focal / depth / (name.chars().count() as f32 * universe_engine::frame::GLYPH);
            let scale = (0.3 * focal / depth / 8.).min(fit).min(2.5);
            if scale >= 0.4 {child.world_text(at.as_dvec3(), name, Color([1.,0.92,0.6,1.]),scale,true);}
        }
    });
    let c = size * 0.5;
    frame.hud_line(c - Vec2::new(6.,0.), c + Vec2::new(6.,0.), LABEL);
    frame.hud_line(c - Vec2::new(0.,6.), c + Vec2::new(0.,6.), LABEL);
    frame.text(Vec2::new(12.,10.), &format!("{place}   WALK"), LABEL);
    frame.text_scaled(Vec2::new(12.,size.y-18.), "WASD WALK - DRAG OR ARROWS LOOK - SPACE JUMP - SHIFT RUN - FORWARD CLIMBS SHAFTS - ESC BACK TO STUDIO", LABEL.scale(0.7),0.7);
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe_engine::glam::DVec3;
    use universe_sim::world::walk::WalkMesh;

    fn floor(y: f64) -> Vec<[DVec3; 3]> {
        let (a, b, c, d) = (DVec3::new(-10., y, -10.), DVec3::new(10., y, -10.),
            DVec3::new(10., y, 10.), DVec3::new(-10., y, 10.));
        vec![[a, b, c], [a, c, d]]
    }

    fn stand(authored: &[[DVec3; 3]], native: &[[DVec3; 3]]) -> Stand {
        Stand {
            mesh: WalkMesh::new(authored), native_mesh: Some(Arc::new(WalkMesh::new(native))),
            // Collider vertices are already centred; this offset belongs only to rendering.
            native_model: Some(("unused.glb".into(), DVec3::new(17., 8., -3.))),
            scene: Default::default(), hatches: vec![],
            clock: universe_sim::clocks::Runtime::new(&universe_sim::clocks::Subscriptions::of(&[universe_sim::world::registry::ClockKey::Realtime]), 1.0, universe_sim::universe::TICK_BUDGET as u32).expect("registry realtime clock"), jump_pending: 0.,
            faces: vec![], equipment: vec![], rooms: vec![], airlock_rooms: vec![],
            labels: vec![], members: vec![], shafts: vec![],
            feet: DVec3::ZERO, velocity: DVec3::ZERO, yaw: 0., pitch: 0.,
        }
    }

    #[test]
    fn floor_pick_uses_nearest_native_or_authored_floor_without_double_offset() {
        let s = stand(&floor(-4.), &floor(2.));
        let p = stand_spot(&s, Vec3::new(0., 5., 0.), Vec3::NEG_Y).unwrap();
        assert!(p.abs_diff_eq(DVec3::new(0., 2.05, 0.), 1e-6));
        let s = stand(&floor(3.), &floor(2.));
        assert!((stand_spot(&s, Vec3::new(0., 5., 0.), Vec3::NEG_Y).unwrap().y - 3.05).abs() < 1e-6);
        assert!(stand_spot(&s, Vec3::ZERO, Vec3::ZERO).is_none());
    }

    fn hatch() -> StandHatch {
        let leaf = Leaf {
            outline: [(-0.3, 0.), (0.3, 0.), (0.5, 0.2), (0.5, 1.8),
                (0.3, 2.), (-0.3, 2.), (-0.5, 1.8), (-0.5, 0.2)]
                .map(|(x, y)| Vec3::new(x, y, 0.)).to_vec(),
            window: vec![Vec3::new(-0.1, 0.9, 0.), Vec3::new(0.1, 0.9, 0.),
                Vec3::new(0.1, 1.1, 0.), Vec3::new(-0.1, 1.1, 0.)],
            middle: Vec3::Y, open: Vec3::X * 1.2,
        };
        let mesh = crate::models::hatches(std::slice::from_ref(&leaf)).pop().unwrap();
        StandHatch::new(universe_engine::scene::ObjectId(17), leaf, mesh)
    }

    fn hatch_observer(distance: f64) -> DVec3 {
        DVec3::new(0., 1. - universe_sim::world::crew::EYE, distance)
    }

    #[test]
    fn moving_leaf_collider_preserves_polygon_corners_and_matches_visual_shift() {
        let mut s = stand(&floor(-4.), &[]);
        s.hatches.push(hatch());
        let ray = |colliders: &[_], x, y| universe_sim::world::walk::ray(
            colliders, DVec3::new(x, y, 1.), DVec3::NEG_Z, 2.);
        let closed = stand_colliders(&s.mesh, None, &s.hatches, hatch_observer(3.));
        assert!(ray(&closed, 0., 1.).is_some(), "closed leaf and its glass are solid");
        assert!(ray(&closed, 0.49, 0.01).is_none(), "do not box-fill the chamfered corner");
        let feet = hatch_observer(1.);
        let open = stand_colliders(&s.mesh, None, &s.hatches, feet);
        assert!(ray(&open, 0., 1.).is_none(), "slid leaf clears its doorway");
        assert!(ray(&open, s.hatches[0].shift(feet).x, 1.).is_some(), "collision follows visible leaf");
    }

}
