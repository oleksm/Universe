//! Retained, isolated presentation graph. No domain simulation or asset IO.
use crate::{
    Color, Frame,
    model::{Mesh, Transform},
    pbr::PbrModel,
};
use glam::{DQuat, DVec3};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub position: DVec3,
    pub rotation: DQuat,
    pub scale: f64,
}
impl Default for Pose {
    fn default() -> Self {
        Self {
            position: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            scale: 1.0,
        }
    }
}
impl Pose {
    pub fn compose(self, child: Self) -> Self {
        Self {
            position: self.position + self.rotation * (child.position * self.scale),
            rotation: self.rotation * child.rotation,
            scale: self.scale * child.scale,
        }
    }
    pub fn transform(self) -> Transform {
        Transform {
            position: self.position,
            rotation: self.rotation.as_quat(),
            scale: self.scale,
        }
    }
    fn valid(self) -> bool {
        self.position.is_finite()
            && self.rotation.is_finite()
            && (self.rotation.length_squared() - 1.).abs() < 1e-8
            && self.scale.is_finite()
            && self.scale > 0.
    }
}

#[derive(Clone)]
pub enum Geometry {
    Model {
        model: PbrModel,
        part: u8,
    },
    Mesh {
        mesh: Mesh,
        line: Color,
        fill: Color,
    },
}
#[derive(Clone)]
pub struct Node {
    pub parent: Option<ObjectId>,
    pub pose: Pose,
    pub geometry: Option<Geometry>,
    /// Authoring pick proxy, in local coordinates; separate from physical colliders.
    pub bounds: Option<[DVec3; 2]>,
    pub layers: u64,
    pub visible: bool,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            parent: None,
            pose: Pose::default(),
            geometry: None,
            bounds: None,
            layers: u64::MAX,
            visible: true,
        }
    }
}
#[derive(Default)]
pub struct Scene {
    nodes: BTreeMap<ObjectId, Node>,
    revision: u64,
}
impl Scene {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn get(&self, id: ObjectId) -> Option<&Node> {
        self.nodes.get(&id)
    }
    /// Atomic upsert: missing parents, cycles and invalid transforms are refused.
    pub fn set(&mut self, id: ObjectId, node: Node) -> Result<(), &'static str> {
        if !node.pose.valid() {
            return Err("invalid scene pose");
        }
        if let Some([lo, hi]) = node.bounds {
            if !lo.is_finite() || !hi.is_finite() || !lo.cmple(hi).all() {
                return Err("invalid bounds");
            }
        }
        let mut parent = node.parent;
        while let Some(p) = parent {
            if p == id {
                return Err("scene cycle");
            }
            parent = self.nodes.get(&p).ok_or("missing parent")?.parent;
        }
        self.nodes.insert(id, node);
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    /// Remove a complete subtree so no orphan or retained asset lease survives it.
    pub fn remove(&mut self, id: ObjectId) {
        let children: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.parent == Some(id))
            .map(|(id, _)| *id)
            .collect();
        for child in children {
            self.remove(child);
        }
        if self.nodes.remove(&id).is_some() {
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn world_pose(&self, id: ObjectId) -> Option<Pose> {
        let node = self.nodes.get(&id)?;
        Some(match node.parent {
            Some(p) => self.world_pose(p)?.compose(node.pose),
            None => node.pose,
        })
    }
    fn shown(&self, id: ObjectId) -> bool {
        let n = &self.nodes[&id];
        n.visible && n.parent.is_none_or(|p| self.shown(p))
    }
    pub fn draw(&self, frame: &mut Frame, layers: u64) {
        for (&id, node) in &self.nodes {
            if !self.shown(id) || node.layers & layers == 0 {
                continue;
            }
            let t = self.world_pose(id).unwrap().transform();
            match &node.geometry {
                Some(Geometry::Model { model, part }) => frame.model_pbr_part(model, &t, *part),
                Some(Geometry::Mesh { mesh, line, fill }) => {
                    frame.model_shaded(mesh, &t, *line, *fill)
                }
                None => (),
            }
        }
    }
    /// Ray/OBB proxy hits sorted by world distance. Never a physics collision query.
    pub fn pick(&self, ray: crate::viewport::Ray, layers: u64) -> Vec<(ObjectId, f64)> {
        let mut hits = Vec::new();
        for (&id, n) in &self.nodes {
            if !self.shown(id) || n.layers & layers == 0 {
                continue;
            }
            let Some([lo, hi]) = n.bounds else {
                continue;
            };
            let p = self.world_pose(id).unwrap();
            let origin = p.rotation.inverse() * (ray.origin - p.position) / p.scale;
            let direction = p.rotation.inverse() * ray.direction / p.scale;
            let (mut near, mut far) = (0f64, f64::INFINITY);
            for k in 0..3 {
                if direction[k] == 0. {
                    if origin[k] < lo[k] || origin[k] > hi[k] {
                        far = -1.;
                    }
                } else {
                    let a = (lo[k] - origin[k]) / direction[k];
                    let b = (hi[k] - origin[k]) / direction[k];
                    near = near.max(a.min(b));
                    far = far.min(a.max(b));
                }
            }
            if near <= far {
                hits.push((id, near));
            }
        }
        hits.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        hits
    }
}

/// Generic fixed-step scheduling only. Caller runs the existing domain tick.
/// Excess time remains queued, never silently discarded.
pub struct TickClock {
    step: f64,
    max_steps: u32,
    pending: f64,
    time: f64,
}
impl TickClock {
    pub fn new(step: f64, max_steps: u32) -> Result<Self, &'static str> {
        if !step.is_finite() || step <= 0. || max_steps == 0 {
            return Err("invalid tick schedule");
        }
        Ok(Self {
            step,
            max_steps,
            pending: 0.,
            time: 0.,
        })
    }
    pub fn pending(&self) -> f64 {
        self.pending
    }
    pub fn advance(
        &mut self,
        dt: f64,
        mut tick: impl FnMut(f64, f64),
    ) -> Result<u32, &'static str> {
        if !dt.is_finite() || dt < 0. || !(self.pending + dt).is_finite() {
            return Err("invalid elapsed time");
        }
        self.pending += dt;
        let mut count = 0;
        while self.pending >= self.step && count < self.max_steps {
            tick(self.time, self.step);
            self.time += self.step;
            self.pending -= self.step;
            count += 1;
        }
        Ok(count)
    }
}
