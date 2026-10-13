//! Actual selected input adapter. It never requests or reconstructs absent neighbours.
use crate::{
    contact_core::{Chart, grid::Triangle},
    crease::{area, rect},
};
use glam::{DVec2, DVec3, Vec3};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};
#[derive(Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub selected: Option<Vec<Binding>>,
    pub source_sha256: String,
    pub radius_m: f64,
    pub eye: [f64; 3],
    pub generation: u64,
    pub patches: Vec<Patch>,
}
#[derive(Deserialize)]
pub struct Binding {
    pub key: [u32; 4],
    pub mesh_id: u64,
    pub partial: bool,
}
impl Snapshot {
    /// Diagnostic JSON copies, additional to the existing source/resident meshes.
    pub fn copy_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.source_sha256.capacity()
            + self
                .selected
                .as_ref()
                .map_or(0, |v| v.capacity() * std::mem::size_of::<Binding>())
            + self.patches.capacity() * std::mem::size_of::<Patch>()
            + self
                .patches
                .iter()
                .map(|p| {
                    p.positions.capacity() * 12 + p.morph.capacity() * 16 + p.faces.capacity() * 12
                })
                .sum::<usize>()
    }
}
#[derive(Deserialize)]
pub struct Patch {
    pub key: [u32; 4],
    pub mesh_id: u64,
    pub origin: [f64; 3],
    pub partial: bool,
    pub positions: Vec<[f32; 3]>,
    pub morph: Vec<[f32; 4]>,
    pub faces: Vec<[u32; 3]>,
}
/// Read-only resident geometry. Implementations must expose valid indices
/// `0..vertex_count()` for both position and morph, and keep all data stable
/// throughout a conversion. Interleaved client vertices need no array copies.
pub trait RetainedMesh: Send + Sync {
    fn position(&self, i: usize) -> [f32; 3];
    fn morph(&self, i: usize) -> [f32; 4];
    fn faces(&self) -> &[[u32; 3]];
    fn vertex_count(&self) -> usize;
}

/// Strongly retained client geometry and its selected identity.
#[derive(Clone)]
pub struct RetainedPatch {
    pub key: [u32; 4],
    pub mesh_id: u64,
    pub origin: [f64; 3],
    pub partial: bool,
    pub mesh: Arc<dyn RetainedMesh>,
}

/// A view over existing geometry; neither arrays nor the mesh are cloned.
#[derive(Clone, Copy)]
pub struct BorrowedRetainedPatch<'a> {
    pub key: [u32; 4],
    pub mesh_id: u64,
    pub origin: [f64; 3],
    pub partial: bool,
    pub mesh: &'a dyn RetainedMesh,
}

impl RetainedPatch {
    pub fn borrowed(&self) -> BorrowedRetainedPatch<'_> {
        BorrowedRetainedPatch {
            key: self.key,
            mesh_id: self.mesh_id,
            origin: self.origin,
            partial: self.partial,
            mesh: self.mesh.as_ref(),
        }
    }
}

impl RetainedMesh for Patch {
    fn position(&self, i: usize) -> [f32; 3] {
        self.positions[i]
    }
    fn morph(&self, i: usize) -> [f32; 4] {
        self.morph[i]
    }
    fn faces(&self) -> &[[u32; 3]] {
        &self.faces
    }
    fn vertex_count(&self) -> usize {
        self.positions.len()
    }
}

/// Convert a subset of strongly retained meshes, bound against the complete
/// selected metadata. No source vertex/face arrays are copied.
pub fn triangles_retained(
    radius_m: f64,
    eye: [f64; 3],
    generation: u64,
    selected: &[Binding],
    patches: &[RetainedPatch],
    chart: &Chart,
) -> Result<(Vec<Triangle>, usize), String> {
    triangles_impl(
        radius_m,
        eye,
        generation,
        Some(selected),
        patches.iter().map(|p| (p.borrowed(), true)),
        chart,
    )
}

/// Borrowed equivalent of `triangles_retained`, with the same mandatory full
/// selected metadata and identity/ancestor validation.
pub fn triangles_borrowed(
    radius_m: f64,
    eye: [f64; 3],
    generation: u64,
    selected: &[Binding],
    patches: &[BorrowedRetainedPatch<'_>],
    chart: &Chart,
) -> Result<(Vec<Triangle>, usize), String> {
    triangles_impl(
        radius_m,
        eye,
        generation,
        Some(selected),
        patches.iter().map(|p| (*p, true)),
        chart,
    )
}

/// Legacy JSON entry point; preserves optional selected metadata and validation.
pub fn triangles(snapshot: &Snapshot, chart: &Chart) -> Result<(Vec<Triangle>, usize), String> {
    triangles_impl(
        snapshot.radius_m,
        snapshot.eye,
        snapshot.generation,
        snapshot.selected.as_deref(),
        snapshot.patches.iter().map(|p| {
            (
                BorrowedRetainedPatch {
                    key: p.key,
                    mesh_id: p.mesh_id,
                    origin: p.origin,
                    partial: p.partial,
                    mesh: p,
                },
                p.morph.len() == p.positions.len(),
            )
        }),
        chart,
    )
}

fn triangles_impl<'a>(
    radius_m: f64,
    eye: [f64; 3],
    generation: u64,
    selected: Option<&[Binding]>,
    patches: impl Clone + ExactSizeIterator<Item = (BorrowedRetainedPatch<'a>, bool)>,
    chart: &Chart,
) -> Result<(Vec<Triangle>, usize), String> {
    let patch_count = patches.len();
    if radius_m != chart.r
        || generation == 0
        || patch_count == 0
        || patch_count > 3000
        || !DVec3::from_array(eye).is_finite()
    {
        return Err("invalid resident snapshot binding/cap".into());
    }
    let mut keys = BTreeSet::new();
    for (p, _) in patches.clone() {
        let [f, l, x, y] = p.key;
        if f >= 6
            || l > 19
            || x >= 1u32 << l
            || y >= 1u32 << l
            || !keys.insert(p.key)
            || p.partial
            || p.mesh_id == 0
        {
            return Err("invalid/partial/duplicate resident patch".into());
        }
    }
    if let Some(selected) = selected {
        if selected.is_empty() || selected.len() > 3000 {
            return Err("selected metadata cap".into());
        }
        keys.clear();
        let mut bindings = std::collections::BTreeMap::new();
        for b in selected {
            let [f, l, x, y] = b.key;
            if f >= 6
                || l > 19
                || x >= 1u32 << l
                || y >= 1u32 << l
                || b.mesh_id == 0
                || !keys.insert(b.key)
            {
                return Err("invalid selected metadata".into());
            }
            bindings.insert(b.key, (b.mesh_id, b.partial));
        }
        for (p, _) in patches.clone() {
            if bindings.get(&p.key) != Some(&(p.mesh_id, p.partial)) {
                return Err("geometry does not match actual selected metadata".into());
            }
        }
    }
    for &[f, l, x, y] in &keys {
        for parent in 0..l {
            if keys.contains(&[f, parent, x >> (l - parent), y >> (l - parent)]) {
                return Err("selected ancestor/descendant overlap".into());
            }
        }
    }
    let eye = DVec3::from_array(eye);
    let mut out = Vec::new();
    for (p, shape_valid) in patches {
        let origin = DVec3::from_array(p.origin);
        if !origin.is_finite()
            || !(289..=425).contains(&p.mesh.vertex_count())
            || !shape_valid
            || p.mesh.faces().len() != 512
        {
            return Err("resident grid shape".into());
        }
        let mut vertices = Vec::with_capacity(289);
        for i in 0..289 {
            let raw = Vec3::from_array(p.mesh.position(i));
            let m = p.mesh.morph(i);
            if !raw.is_finite() || m.iter().any(|x| !x.is_finite()) || m[3] <= 0. {
                return Err("invalid resident vertex/morph".into());
            }
            let far = (2.4 * 2. * m[3] as f64 * 0.95) as f32;
            let distance = (origin + raw.as_dvec3() - eye).length() as f32;
            let t = ((distance - far * 0.6) / (far * 0.4)).clamp(0., 1.);
            let v = raw + Vec3::new(m[0], m[1], m[2]) * (t * t * (3. - 2. * t));
            vertices.push(origin + v.as_dvec3());
        }
        let mut seen = BTreeSet::new();
        for ids in p.mesh.faces() {
            if ids.iter().any(|&i| i >= 289) || !seen.insert(*ids) {
                return Err("invalid resident grid face".into());
            }
            let world = ids.map(|i| vertices[i as usize]);
            // Ignore the opposite hemisphere, never fabricate support for it.
            if world.iter().any(|p| p.dot(chart.c) <= 0.) {
                continue;
            }
            let uv = world
                .iter()
                .map(|p| chart.project(*p))
                .collect::<Result<Vec<_>, _>>()?;
            if area(&rect(uv.clone(), [-64., 64., -64., 64.])) < 1e-12 {
                continue;
            }
            if out.len() >= 32768 {
                return Err("retained triangle cap".into());
            }
            let lo = uv
                .iter()
                .copied()
                .fold(DVec2::splat(f64::INFINITY), DVec2::min);
            let hi = uv
                .iter()
                .copied()
                .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
            let [f, l, x, y] = p.key;
            out.push(Triangle {
                tags: ids.map(|i| {
                    [
                        f,
                        (x * 16 + i % 17) << (19 - l),
                        (y * 16 + i / 17) << (19 - l),
                    ]
                }),
                patch: p.key,
                origin,
                trace: None,
                world,
                uv,
                lo,
                hi,
            });
        }
    }
    if out.is_empty() {
        return Err("no actual resident support".into());
    }
    Ok((out, patch_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subset_cannot_hide_changed_identity_or_selected_parent() {
        let radius_m = 6281370.;
        let positions = (0..289)
            .map(|i| [0., (i / 17) as f32, -((i % 17) as f32)])
            .collect();
        let faces = (0..16)
            .flat_map(|j| {
                (0..16).flat_map(move |i| {
                    let a = j * 17 + i;
                    [[a, a + 1, a + 18], [a, a + 18, a + 17]]
                })
            })
            .collect();
        let patch = Patch {
            key: [0, 19, 0, 0],
            mesh_id: 1,
            origin: [radius_m, 0., 0.],
            partial: false,
            positions,
            morph: vec![[0., 0., 0., 100.]; 289],
            faces,
        };
        let mut snapshot = Snapshot {
            source_sha256: String::new(),
            radius_m,
            eye: [radius_m + 1.7, 0., 0.],
            generation: 1,
            selected: Some(vec![Binding {
                key: patch.key,
                mesh_id: 1,
                partial: false,
            }]),
            patches: vec![patch],
        };
        let chart = Chart::new(DVec3::X, radius_m);
        assert_eq!(triangles(&snapshot, &chart).unwrap().0.len(), 512);
        let expected = triangles(&snapshot, &chart).unwrap().0;
        let patch = &snapshot.patches[0];
        let borrowed = [BorrowedRetainedPatch {
            key: patch.key,
            mesh_id: patch.mesh_id,
            origin: patch.origin,
            partial: patch.partial,
            mesh: patch,
        }];
        assert!(std::ptr::eq(
            borrowed[0].mesh.faces(),
            patch.faces.as_slice()
        ));
        let actual = triangles_borrowed(
            radius_m,
            snapshot.eye,
            1,
            snapshot.selected.as_deref().unwrap(),
            &borrowed,
            &chart,
        )
        .unwrap()
        .0;
        for (a, b) in actual.iter().zip(&expected) {
            assert_eq!(a.world, b.world);
            assert_eq!(a.uv, b.uv);
            assert_eq!(a.tags, b.tags);
            assert_eq!(a.patch, b.patch);
        }
        assert_eq!(actual.len(), expected.len());
        snapshot.selected.as_mut().unwrap()[0].mesh_id = 2;
        assert!(
            matches!(triangles(&snapshot,&chart),Err(e) if e=="geometry does not match actual selected metadata")
        );
        assert!(matches!(triangles_borrowed(radius_m, snapshot.eye, 1,
            snapshot.selected.as_deref().unwrap(), &borrowed, &chart),
            Err(e) if e == "geometry does not match actual selected metadata"));
        snapshot.selected.as_mut().unwrap()[0].mesh_id = 1;
        snapshot.selected.as_mut().unwrap().push(Binding {
            key: [0, 18, 0, 0],
            mesh_id: 2,
            partial: false,
        });
        assert!(
            matches!(triangles(&snapshot,&chart),Err(e) if e=="selected ancestor/descendant overlap")
        );
        assert!(matches!(triangles_borrowed(radius_m, snapshot.eye, 1,
            snapshot.selected.as_deref().unwrap(), &borrowed, &chart),
            Err(e) if e == "selected ancestor/descendant overlap"));
        snapshot.selected.as_mut().unwrap().pop();
        let patch = snapshot.patches.pop().unwrap();
        let retained = [RetainedPatch {
            key: patch.key,
            mesh_id: patch.mesh_id,
            origin: patch.origin,
            partial: patch.partial,
            mesh: Arc::new(patch),
        }];
        let actual = triangles_retained(
            radius_m,
            snapshot.eye,
            1,
            snapshot.selected.as_deref().unwrap(),
            &retained,
            &chart,
        )
        .unwrap()
        .0;
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(&expected) {
            assert_eq!(a.world, b.world);
            assert_eq!(a.uv, b.uv);
            assert_eq!(a.tags, b.tags);
        }
    }
}
