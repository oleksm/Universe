//! Actual resident inputs for the optional contact worker. No terrain queries or rebuilt grids.
use super::{Key, Patch};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use universe_engine::{Mesh, glam::DVec3};

/// Identifies an immutable input vertex set, including the eye used by shader morph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub source: (usize, usize, u64),
    pub radius: u64,
    pub eye: [u64; 3],
    pub selected: Vec<(Key, u64)>,
}
#[derive(Clone)]
pub(crate) struct ResidentPatch {
    pub key: Key,
    pub mesh: Mesh,
    pub origin: DVec3,
    pub partial: bool,
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub generation: u64,
    pub cancellation: Arc<AtomicU64>,
    pub stamp: Stamp,
    pub radius: f64,
    pub eye: DVec3,
    pub patches: Vec<ResidentPatch>,
}
impl Snapshot {
    pub fn current(&self) -> bool {
        self.cancellation.load(Ordering::Acquire) == self.generation
    }
    pub fn complete(&self) -> bool {
        self.current() && !self.patches.is_empty() && self.patches.iter().all(|p| !p.partial)
    }
}
#[derive(Default)]
pub(crate) struct ResidentInputs {
    generation: Arc<AtomicU64>,
    snapshot: Option<Arc<Snapshot>>,
}
impl ResidentInputs {
    pub fn invalidate(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.snapshot = None;
    }
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.snapshot.clone()
    }
    /// Only `shown`, the real post-fallback draw selection, may be supplied.
    pub fn capture(
        &mut self,
        source: (usize, usize, u64),
        radius: f64,
        eye: DVec3,
        shown: &[Key],
        patches: &HashMap<Key, Patch>,
    ) {
        if !radius.is_finite()
            || radius <= 0.
            || !eye.is_finite()
            || shown.is_empty()
            || shown.len() > super::MAX_PATCHES
        {
            self.invalidate();
            return;
        }
        let mut selected = Vec::with_capacity(shown.len());
        for key in shown {
            let Some(patch) = patches.get(key) else {
                self.invalidate();
                return;
            };
            if (key.system, key.body) != (source.0, source.1) || !patch.origin.is_finite() {
                self.invalidate();
                return;
            }
            selected.push((*key, patch.mesh.id()));
        }
        selected.sort_unstable_by_key(|(k, _)| (k.system, k.body, k.face, k.level, k.x, k.y));
        if selected.windows(2).any(|w| w[0].0 == w[1].0) {
            self.invalidate();
            return;
        }
        // Mixed parent/child overdraw is real input, never silently replaced with an ideal owner.
        let keys: std::collections::HashSet<_> = shown.iter().copied().collect();
        for key in shown {
            let mut ancestor = key.parent();
            while let Some(parent) = ancestor {
                if keys.contains(&parent) {
                    self.invalidate();
                    return;
                }
                ancestor = parent.parent();
            }
        }
        let stamp = Stamp {
            source,
            radius: radius.to_bits(),
            eye: eye.to_array().map(f64::to_bits),
            selected,
        };
        // Readiness changes when partial patches are remade (a new Mesh.id), not on draw-counter churn.
        if self.snapshot.as_ref().is_some_and(|s| s.stamp == stamp) {
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let resident = stamp
            .selected
            .iter()
            .map(|(key, _)| {
                let p = &patches[key];
                ResidentPatch {
                    key: *key,
                    mesh: p.mesh.clone(),
                    origin: p.origin,
                    partial: p.partial,
                }
            })
            .collect();
        self.snapshot = Some(Arc::new(Snapshot {
            generation,
            cancellation: self.generation.clone(),
            stamp,
            radius,
            eye,
            patches: resident,
        }));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use universe_engine::{WireModel, glam::Vec3};
    fn key(level: u8, x: u32) -> Key {
        Key {
            system: 0,
            body: 1,
            face: 0,
            level,
            x,
            y: 0,
        }
    }
    fn patch(partial: bool) -> Patch {
        Patch {
            mesh: WireModel {
                positions: vec![Vec3::ZERO],
                ..Default::default()
            }
            .into(),
            origin: DVec3::X * 100.,
            partial,
            used: 0,
        }
    }
    #[test]
    fn resident_identity_eye_source_and_remake_cancel_old_inputs() {
        let k = key(1, 0);
        let mut patches = HashMap::from([(k, patch(false))]);
        let mut inputs = ResidentInputs::default();
        inputs.capture((0, 1, 4), 100., DVec3::X * 101., &[k], &patches);
        let first = inputs.snapshot().unwrap();
        assert!(first.complete());
        assert_eq!(first.patches[0].mesh.id(), patches[&k].mesh.id());
        inputs.capture((0, 1, 4), 100., DVec3::X * 101., &[k], &patches);
        assert!(Arc::ptr_eq(&first, &inputs.snapshot().unwrap()));
        inputs.capture((0, 1, 4), 100., DVec3::X * 101.001, &[k], &patches);
        assert!(!first.current());
        let second = inputs.snapshot().unwrap();
        patches.insert(k, patch(false));
        inputs.capture((0, 1, 4), 100., DVec3::X * 101.001, &[k], &patches);
        assert!(!second.current());
        let third = inputs.snapshot().unwrap();
        inputs.capture((0, 1, 5), 100., DVec3::X * 101.001, &[k], &patches);
        assert!(!third.current());
    }
    #[test]
    fn actual_missing_partial_or_overlapping_support_cannot_be_complete() {
        let parent = key(0, 0);
        let child = key(1, 0);
        let mut patches = HashMap::from([(parent, patch(false)), (child, patch(true))]);
        let mut inputs = ResidentInputs::default();
        inputs.capture((0, 1, 1), 100., DVec3::X * 101., &[child], &patches);
        assert!(!inputs.snapshot().unwrap().complete());
        inputs.capture((0, 1, 1), 100., DVec3::X * 101., &[parent, child], &patches);
        assert!(inputs.snapshot().is_none());
        patches.remove(&child);
        inputs.capture((0, 1, 1), 100., DVec3::X * 101., &[child], &patches);
        assert!(inputs.snapshot().is_none());
    }
}

/// Zero-copy bridge to the world-owned worker. No engine type occurs in the trait.
pub(crate) trait RetainedMesh: Send + Sync {
    fn vertex_count(&self) -> usize;
    fn position(&self, index: usize) -> [f32; 3];
    fn morph(&self, index: usize) -> [f32; 4];
    fn faces(&self) -> &[[u32; 3]];
}
impl RetainedMesh for ResidentPatch {
    fn vertex_count(&self) -> usize {
        self.mesh.positions.len()
    }
    fn position(&self, index: usize) -> [f32; 3] {
        self.mesh.positions[index].to_array()
    }
    fn morph(&self, index: usize) -> [f32; 4] {
        self.mesh.colors[index]
    }
    fn faces(&self) -> &[[u32; 3]] {
        &self.mesh.faces[..self
            .mesh
            .faces
            .len()
            .min((super::GRID * super::GRID * 2) as usize)]
    }
}
impl Snapshot {
    /// Conservative nearby actual-mesh selection, then one ring of real incident patches.
    /// This is an export subset, NOT proof that correction/publication dependency closure passed.
    fn nearby_indices(&self) -> Vec<usize> {
        let centre = self.eye.normalize();
        let east = centre.any_orthonormal_vector();
        let north = centre.cross(east);
        let mut included = std::collections::HashSet::new();
        for (index, p) in self.patches.iter().enumerate() {
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            let mut front = false;
            let mut back = false;
            for i in 0..p.vertex_count().min(289) {
                let raw = universe_engine::glam::Vec3::from_array(p.position(i));
                let m = p.morph(i);
                let far = (super::SPLIT * 2. * m[3] as f64 * 0.95) as f32;
                let distance = (p.origin + raw.as_dvec3() - self.eye).length() as f32;
                let t = ((distance - far * 0.6) / (far * 0.4)).clamp(0., 1.);
                let world = p.origin
                    + (raw
                        + universe_engine::glam::Vec3::new(m[0], m[1], m[2])
                            * (t * t * (3. - 2. * t)))
                        .as_dvec3();
                let denom = world.dot(centre);
                if denom <= 0. {
                    back = true;
                    continue;
                }
                front = true;
                let uv = [
                    self.radius * world.dot(east) / denom,
                    self.radius * world.dot(north) / denom,
                ];
                for k in 0..2 {
                    lo[k] = lo[k].min(uv[k]);
                    hi[k] = hi[k].max(uv[k]);
                }
            }
            // 128m half-width is a conservative export margin, not a new contact/core gate.
            if front
                && (back || (lo[0] <= 128. && hi[0] >= -128. && lo[1] <= 128. && hi[1] >= -128.))
            {
                included.insert(index);
            }
        }
        let original: Vec<_> = included.iter().copied().collect();
        let interval = |k: Key| {
            let shift = 19 - k.level;
            [
                k.x << shift,
                (k.x + 1) << shift,
                k.y << shift,
                (k.y + 1) << shift,
            ]
        };
        for &i in &original {
            let a = self.patches[i].key;
            let aa = interval(a);
            for (j, p) in self.patches.iter().enumerate() {
                let b = p.key;
                if a.face != b.face {
                    continue;
                }
                let bb = interval(b);
                if ((aa[1] == bb[0] || aa[0] == bb[1]) && aa[2] <= bb[3] && bb[2] <= aa[3])
                    || ((aa[3] == bb[2] || aa[2] == bb[3]) && aa[0] <= bb[1] && bb[0] <= aa[1])
                {
                    included.insert(j);
                }
            }
        }
        let mut indices: Vec<_> = included.into_iter().collect();
        indices.sort_unstable();
        indices
    }
}
impl Snapshot {
    /// CPU export of actual selected meshes, raw f32 vertices/morphs and grid topology.
    /// Serialization borrows arrays. The caller must provide the already-verified source SHA.
    pub fn write_json(
        &self,
        writer: &mut impl std::io::Write,
        source_sha256: &str,
    ) -> Result<(), String> {
        if source_sha256.len() != 64
            || !source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || !self.current()
        {
            return Err("unbound or stale snapshot".into());
        }
        #[derive(serde::Serialize)]
        struct PatchView<'a> {
            key: [u32; 4],
            mesh_id: u64,
            origin: [f64; 3],
            partial: bool,
            positions: &'a [universe_engine::glam::Vec3],
            morph: &'a [[f32; 4]],
            faces: &'a [[u32; 3]],
        }
        #[derive(serde::Serialize)]
        struct Selected {
            key: [u32; 4],
            mesh_id: u64,
            partial: bool,
        }
        #[derive(serde::Serialize)]
        struct View<'a> {
            format: &'static str,
            source_sha256: &'a str,
            radius_m: f64,
            eye: [f64; 3],
            generation: u64,
            system: usize,
            body: usize,
            map_id: u64,
            patches: Vec<PatchView<'a>>,
            selected: Vec<Selected>,
            scope: &'static str,
        }
        let mut patches = Vec::with_capacity(self.patches.len());
        let indices = self.nearby_indices();
        if indices.is_empty() {
            return Err("no nearby actual support".into());
        }
        for index in indices {
            let p = &self.patches[index];
            if p.vertex_count() < 289
                || p.mesh.colors.len() != p.vertex_count()
                || p.faces().len() != 512
            {
                return Err("invalid actual resident grid".into());
            }
            patches.push(PatchView {
                key: [p.key.face as u32, p.key.level as u32, p.key.x, p.key.y],
                mesh_id: p.mesh.id(),
                origin: p.origin.to_array(),
                partial: p.partial,
                positions: &p.mesh.positions,
                morph: &p.mesh.colors,
                faces: p.faces(),
            });
        }
        let view = View {
            format: "actual-resident-terrain/1",
            source_sha256,
            radius_m: self.radius,
            eye: self.eye.to_array(),
            generation: self.generation,
            system: self.stamp.source.0,
            body: self.stamp.source.1,
            map_id: self.stamp.source.2,
            patches,
            selected: self
                .patches
                .iter()
                .map(|p| Selected {
                    key: [p.key.face as u32, p.key.level as u32, p.key.x, p.key.y],
                    mesh_id: p.mesh.id(),
                    partial: p.partial,
                })
                .collect(),
            scope: "CPU actual selection, nearby raw geometry plus real same-face incident ring; missing finite-owner closure must refuse; no GPU submission/publication",
        };
        serde_json::to_writer(writer, &view).map_err(|e| e.to_string())?;
        if !self.current() {
            return Err("snapshot cancelled during export".into());
        }
        Ok(())
    }
}
