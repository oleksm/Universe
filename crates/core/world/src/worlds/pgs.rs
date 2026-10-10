//! PGS1 revision 3, canonical version-1 roots. Positions stay on their stored chord
//! planes. Unsupported capabilities are refused; absent water/categories are unknown.
//! This reader does not install a world or reinterpret the registry's PTL2 packages.
use glam::DVec3;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

const NONE: u32 = u32::MAX;
const TIE: f64 = 1e-12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Water {
    Unknown,
    Dry,
    Wet {
        body: u32,
        level_m: f64,
        depth_m: f64,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub face: u32,
    pub height_m: f64,
    pub water: Water,
    /// Owner-face (rock, pattern) IDs in the header vocabularies. None means unknown;
    /// Some((0, 0)) means explicitly unassigned, not basalt or missing data.
    pub categories: Option<(u16, u16)>,
}

/// Analytic derivative of the selected owner's radial terrain surface.
/// Gradient is metres per radian on the unit sphere; slope uses R + height.
#[derive(Clone, Copy, Debug)]
pub struct Differential {
    pub gradient: DVec3,
    pub normal: DVec3,
    pub slope: f64,
}

#[derive(Clone, Debug)]
struct Point {
    position: DVec3,
    height: f64,
}
#[derive(Clone, Debug)]
struct Face {
    vertices: [usize; 3],
    neighbours: [usize; 3],
    body: u32,
    categories: (u16, u16),
    edges: [DVec3; 3],
    normal: DVec3,
    plane: f64,
}
#[derive(Clone, Debug)]
struct Node {
    centre: DVec3,
    face: usize,
    axis: usize,
    children: [Option<usize>; 2],
}

#[derive(Clone, Debug)]
pub struct Surface {
    pub radius_m: f64,
    pub capabilities: u32,
    /// Body key, generator id/version/JSON, source hash, rock/pattern vocabularies.
    pub provenance: [String; 7],
    points: Vec<Point>,
    faces: Vec<Face>,
    bodies: BTreeMap<u32, f64>,
    tree: Vec<Node>,
    max: f64,
}

struct Bytes<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("byte range overflow")?;
        let b = self.data.get(self.at..end).ok_or("truncated PGS1")?;
        self.at = end;
        Ok(b)
    }
    fn zero(&mut self, n: usize) -> Result<(), String> {
        if self.take(n)?.iter().any(|&b| b != 0) {
            return Err("nonzero reserved bytes or padding".into());
        }
        Ok(())
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<usize, String> {
        usize::try_from(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
            .map_err(|_| "count exceeds address space".into())
    }
    fn f64(&mut self) -> Result<f64, String> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn align(&mut self) -> Result<(), String> {
        self.zero((8 - self.at % 8) % 8)
    }
}
struct Section {
    kind: u32,
    version: u32,
    flags: u32,
    stride: usize,
    offset: usize,
    length: usize,
    count: usize,
    hash: [u8; 32],
}

impl Surface {
    /// Strict canonical root reader, with per-section checksums and closed topology.
    pub fn read(data: &[u8]) -> Result<Self, String> {
        let mut b = Bytes { data, at: 0 };
        if b.take(4)? != b"PGS1" || b.take(2)? != [1, 0] {
            return Err("expected PGS1 version 1".into());
        }
        b.zero(2)?;
        let capabilities = b.u32()?;
        if capabilities & 1 == 0 || capabilities & !35 != 0 {
            return Err(
                "unsupported PGS1 capabilities (canonical terrain/water/category roots only)".into(),
            );
        }
        b.zero(12)?; // root depth, reserved bytes, tile i/j
        let radius_m = b.f64()?;
        if !radius_m.is_finite() || radius_m <= 0.0 {
            return Err("invalid radius".into());
        }
        if b.u32()? != 7 {
            return Err("expected seven provenance strings".into());
        }
        let mut provenance: [String; 7] = Default::default();
        for s in &mut provenance {
            let n = b.u32()? as usize;
            *s = std::str::from_utf8(b.take(n)?)
                .map_err(|_| "invalid UTF-8 provenance")?
                .to_owned();
        }
        let params: serde_json::Value = serde_json::from_str(&provenance[3])
            .map_err(|_| "invalid generator parameters JSON")?;
        if !params.is_object()
            || provenance[4].len() != 64
            || !provenance[4]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("invalid generator parameters or source hash".into());
        }
        if capabilities & 32 != 0 {
            // Labels are opaque IDs qualified by these namespaces, not material enums.
            for vocabulary in &provenance[5..7] {
                let valid = vocabulary.rsplit_once('@').is_some_and(|(name, version)| {
                    !name.is_empty() && !name.chars().any(char::is_whitespace)
                        && version.parse::<u32>().is_ok_and(|v| v > 0)
                });
                if !valid { return Err("categories require named versioned vocabularies".into()); }
            }
        }
        b.align()?;
        let count = b.u32()? as usize;
        b.zero(4)?;
        if count > data.len().saturating_sub(b.at) / 72 {
            return Err("section table out of bounds".into());
        }
        let mut sections = Vec::with_capacity(count);
        let mut last = 0;
        for _ in 0..count {
            let kind = b.u32()?;
            if kind <= last {
                return Err("sections must have unique ascending kinds".into());
            }
            last = kind;
            let s = Section {
                kind,
                version: b.u32()?,
                flags: b.u32()?,
                stride: b.u32()? as usize,
                offset: b.u64()?,
                length: b.u64()?,
                count: b.u64()?,
                hash: b.take(32)?.try_into().unwrap(),
            };
            if s.flags & !1 != 0 || s.count > u32::MAX as usize {
                return Err("unsupported flags or count".into());
            }
            if s.stride != 0 && s.count.checked_mul(s.stride) != Some(s.length) {
                return Err("section length differs from count times stride".into());
            }
            sections.push(s);
        }
        let table_end = b.at;
        let mut by_offset: Vec<_> = sections.iter().collect();
        by_offset.sort_by_key(|s| s.offset);
        let mut end = table_end;
        for s in by_offset {
            if s.offset % 8 != 0 || s.offset < end {
                return Err("unaligned or overlapping section".into());
            }
            if s.offset > data.len() || data[end..s.offset].iter().any(|&v| v != 0) {
                return Err("invalid section padding".into());
            }
            end = s.offset.checked_add(s.length).ok_or("section overflow")?;
            let bytes = data.get(s.offset..end).ok_or("section outside file")?;
            if Sha256::digest(bytes).as_slice() != s.hash {
                return Err(format!("section {} checksum mismatch", s.kind));
            }
            if s.kind == 14 {
                return Err("compact points are unsupported".into());
            }
            let stride = match s.kind {
                1 => Some(40),
                2 => Some(32),
                4 => Some(16),
                _ => None,
            };
            if let Some(stride) = stride {
                if s.version != 1 || s.stride != stride {
                    return Err("unsupported section version or stride".into());
                }
            } else if s.flags & 1 != 0 {
                return Err(format!("unsupported required section {}", s.kind));
            }
        }
        if data[end..].iter().any(|&v| v != 0) || data.len() - end > 7 {
            return Err("unexpected trailing bytes".into());
        }
        let section = |kind| -> Result<(&Section, Bytes<'_>), String> {
            let s = sections
                .iter()
                .find(|s| s.kind == kind)
                .ok_or_else(|| format!("missing section {kind}"))?;
            Ok((
                s,
                Bytes {
                    data: &data[s.offset..s.offset + s.length],
                    at: 0,
                },
            ))
        };
        let (s, mut b) = section(1)?;
        if s.count < 4 {
            return Err("too few root points".into());
        }
        let mut points = Vec::with_capacity(s.count);
        for _ in 0..s.count {
            let position = DVec3::new(b.f64()?, b.f64()?, b.f64()?);
            let height = b.f64()?;
            let _source = b.u32()?;
            b.zero(4)?;
            if !position.is_finite()
                || !height.is_finite()
                || !position.length().is_finite()
                || position.length() == 0.0
            {
                return Err("invalid canonical point".into());
            }
            points.push(Point { position, height });
        }
        let mut bodies = BTreeMap::new();
        if capabilities & 2 != 0 {
            let (s, mut b) = section(4)?;
            for _ in 0..s.count {
                let id = b.u32()?;
                let kind = b.take(1)?[0];
                b.zero(3)?;
                let level = b.f64()?;
                if id == NONE
                    || !(1..=2).contains(&kind)
                    || !level.is_finite()
                    || bodies.insert(id, level).is_some()
                {
                    return Err("invalid water body".into());
                }
            }
        } else if sections.iter().any(|s| s.kind == 4) {
            return Err("water section without capability".into());
        }
        let (s, mut b) = section(2)?;
        if s.count < 4 {
            return Err("too few root faces".into());
        }
        let mut faces = Vec::with_capacity(s.count);
        let mut vertex_bodies = HashMap::new();
        let mut area = 0.0;
        for _ in 0..s.count {
            let vertices = [b.u32()? as usize, b.u32()? as usize, b.u32()? as usize];
            let neighbours = [b.u32()? as usize, b.u32()? as usize, b.u32()? as usize];
            let body = b.u32()?;
            let categories = (
                u16::from_le_bytes(b.take(2)?.try_into().unwrap()),
                u16::from_le_bytes(b.take(2)?.try_into().unwrap()),
            );
            if vertices.iter().any(|&v| v >= points.len())
                || neighbours.iter().any(|&n| n >= s.count)
            {
                return Err("point/neighbour index outside root".into());
            }
            let [a, c, d] = vertices.map(|v| points[v].position);
            let normal = (c - a).cross(d - a);
            let plane = normal.dot(a);
            let edges = [a.cross(c), c.cross(d), d.cross(a)];
            if !plane.is_finite() || plane <= 0.0 || edges.iter().any(|e| e.length() == 0.0) {
                return Err("degenerate or inward face".into());
            }
            let [u, v, w] = [a, c, d].map(DVec3::normalize);
            area += 2.0
                * u.dot(v.cross(w))
                    .atan2(1.0 + u.dot(v) + v.dot(w) + w.dot(u));
            if capabilities & 2 != 0 && body != NONE {
                let level = *bodies.get(&body).ok_or("face names missing water body")?;
                for &v in &vertices {
                    if points[v].height > level + 0.001 {
                        return Err("wet face above body level".into());
                    }
                    if vertex_bodies
                        .insert(v, body)
                        .is_some_and(|other| other != body)
                    {
                        return Err("conflicting water bodies at boundary".into());
                    }
                }
            }
            faces.push(Face {
                vertices,
                neighbours,
                body,
                categories,
                edges: edges.map(DVec3::normalize),
                normal,
                plane,
            });
        }
        let mut edges = HashMap::new();
        let mut used = HashSet::new();
        for (i, f) in faces.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (f.vertices[k], f.vertices[(k + 1) % 3]);
                used.insert(a);
                if edges.insert((a, b), (i, k)).is_some() {
                    return Err("duplicate directed edge".into());
                }
            }
        }
        for (&(a, b), &(i, k)) in &edges {
            let &(other, slot) = edges.get(&(b, a)).ok_or("open root edge")?;
            if faces[i].neighbours[k] != other || faces[other].neighbours[slot] != i {
                return Err("nonreciprocal neighbour or wrong shared edge".into());
            }
            if capabilities & 2 != 0 && (faces[i].body == NONE) != (faces[other].body == NONE) {
                let wet = if faces[i].body != NONE {
                    &faces[i]
                } else {
                    &faces[other]
                };
                let level = bodies[&wet.body];
                if [a, b]
                    .iter()
                    .any(|&v| (points[v].height - level).abs() > 0.001)
                {
                    return Err("shore edge not at water level".into());
                }
            }
        }
        let mut seen = HashSet::new();
        let mut todo = vec![0];
        while let Some(i) = todo.pop() {
            if seen.insert(i) {
                todo.extend(faces[i].neighbours);
            }
        }
        if seen.len() != faces.len()
            || used.len() != points.len()
            || points.len() + faces.len() != edges.len() / 2 + 2
            || (area - 4.0 * std::f64::consts::PI).abs() > 1e-7
        {
            return Err("root is not a closed spherical surface".into());
        }
        let max = points
            .iter()
            .map(|p| p.height)
            .chain(bodies.values().copied())
            .fold(f64::NEG_INFINITY, f64::max);
        let mut surface = Self {
            radius_m,
            capabilities,
            provenance,
            points,
            faces,
            bodies,
            tree: Vec::new(),
            max,
        };
        let mut centres: Vec<_> = surface
            .faces
            .iter()
            .enumerate()
            .map(|(i, f)| {
                (
                    (f.vertices
                        .iter()
                        .map(|&v| surface.points[v].position)
                        .sum::<DVec3>())
                    .normalize(),
                    i,
                )
            })
            .collect();
        surface.build_tree(&mut centres, 0);
        Ok(surface)
    }
    pub fn point_count(&self) -> usize {
        self.points.len()
    }
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }
    pub fn max_height(&self) -> f64 {
        self.max
    }

    fn build_tree(&mut self, entries: &mut [(DVec3, usize)], depth: usize) -> Option<usize> {
        if entries.is_empty() {
            return None;
        }
        let axis = depth % 3;
        let mid = entries.len() / 2;
        entries.select_nth_unstable_by(mid, |a, b| a.0[axis].total_cmp(&b.0[axis]));
        let (centre, face) = entries[mid];
        let id = self.tree.len();
        self.tree.push(Node {
            centre,
            face,
            axis,
            children: [None; 2],
        });
        let (left, rest) = entries.split_at_mut(mid);
        let children = [
            self.build_tree(left, depth + 1),
            self.build_tree(&mut rest[1..], depth + 1),
        ];
        self.tree[id].children = children;
        Some(id)
    }
    fn nearest(&self, id: usize, q: DVec3, best: &mut (f64, usize)) {
        let n = &self.tree[id];
        let d = n.centre.distance_squared(q);
        if d < best.0 {
            *best = (d, n.face);
        }
        let delta = q[n.axis] - n.centre[n.axis];
        let side = usize::from(delta >= 0.0);
        if let Some(i) = n.children[side] {
            self.nearest(i, q, best);
        }
        if delta * delta <= best.0 {
            if let Some(i) = n.children[1 - side] {
                self.nearest(i, q, best);
            }
        }
    }
    fn inside(&self, face: usize, q: DVec3) -> bool {
        self.faces[face].edges.iter().all(|e| q.dot(*e) >= -TIE)
    }
    fn height(&self, face: usize, q: DVec3) -> f64 {
        let f = &self.faces[face];
        let [a, b, c] = f.vertices.map(|i| self.points[i].position);
        let p = q * (f.plane / f.normal.dot(q));
        let den = f.normal.length_squared();
        let v = (p - a).cross(c - a).dot(f.normal) / den;
        let w = (b - a).cross(p - a).dot(f.normal) / den;
        (1.0 - v - w) * self.points[f.vertices[0]].height
            + v * self.points[f.vertices[1]].height
            + w * self.points[f.vertices[2]].height
    }
    /// Exact derivative of chord-plane barycentric height, using query's tie owner.
    /// `radius_m` is the radius where the host draws this surface; water is not terrain.
    pub fn query_differential(&self, direction: DVec3, radius_m: f64) -> Result<(Sample, Differential), String> {
        let sample = self.query(direction)?;
        let radius = radius_m + sample.height_m;
        if !radius_m.is_finite() || radius_m <= 0.0 || radius <= 0.0 {
            return Err("invalid differential radius".into());
        }
        let q = direction.normalize();
        let f = &self.faces[sample.face as usize];
        let [a,b,c] = f.vertices.map(|i| &self.points[i]);
        let e1 = b.position - a.position;
        let e2 = c.position - a.position;
        let g = ((b.height-a.height)*e2.cross(f.normal)
            + (c.height-a.height)*f.normal.cross(e1))/f.normal.length_squared();
        let nq = f.normal.dot(q);
        let ambient = (g - f.normal*(g.dot(q)/nq))*(f.plane/nq);
        let gradient = ambient - q*ambient.dot(q);
        Ok((sample, Differential { gradient, normal: (q-gradient/radius).normalize(), slope: gradient.length()/radius }))
    }

    /// Direction need not be normalized. Zero and nonfinite queries are errors.
    /// A runtime centroid index seeds a neighbour walk; boundary traversal visits
    /// the entire tied fan, including a wet face when the terrain owner is dry.
    pub fn query(&self, direction: DVec3) -> Result<Sample, String> {
        let length = direction.length();
        if !direction.is_finite() || !length.is_finite() || length == 0.0 {
            return Err("invalid query direction".into());
        }
        let q = direction / length;
        let mut nearest = (f64::INFINITY, 0);
        self.nearest(0, q, &mut nearest);
        let mut face = nearest.1;
        let mut found = false;
        for _ in 0..self.faces.len().min(128) {
            let f = &self.faces[face];
            let sides = f.edges.map(|e| e.dot(q));
            let k = (0..3)
                .min_by(|&a, &b| sides[a].total_cmp(&sides[b]))
                .unwrap();
            if sides[k] >= -TIE {
                found = true;
                break;
            }
            face = f.neighbours[k];
        }
        if !found {
            face = (0..self.faces.len())
                .find(|&i| self.inside(i, q))
                .ok_or("no containing face")?;
        }
        let mut tied = vec![face];
        let mut at = 0;
        while at < tied.len() {
            let f = &self.faces[tied[at]];
            for k in 0..3 {
                let n = f.neighbours[k];
                if f.edges[k].dot(q).abs() <= TIE && !tied.contains(&n) && self.inside(n, q) {
                    tied.push(n);
                }
            }
            at += 1;
        }
        face = *tied.iter().min().unwrap();
        let height_m = self.height(face, q);
        let water = if self.capabilities & 2 == 0 {
            Water::Unknown
        } else {
            match tied.iter().find(|&&i| self.faces[i].body != NONE) {
                None => Water::Dry,
                Some(&wet) => {
                    let body = self.faces[wet].body;
                    let level_m = self.bodies[&body];
                    let shore = tied.iter().any(|&i| self.faces[i].body == NONE);
                    Water::Wet {
                        body,
                        level_m,
                        depth_m: if shore {
                            0.0
                        } else {
                            (level_m - height_m).max(0.0)
                        },
                    }
                }
            }
        };
        Ok(Sample {
            face: face as u32,
            height_m,
            water,
            categories: (self.capabilities & 32 != 0).then_some(self.faces[face].categories),
        })
    }
}

/// Immutable sources keyed by registry body key. Supplied by the host before
/// systems are generated, so physics and separately generated charts agree.
pub type Sources = HashMap<String, std::sync::Arc<Surface>>;

pub(crate) fn apply_sources(system: &mut crate::system::StarSystem, sources: &Sources) {
    for body in &mut system.bodies {
        if let Some(source) = sources.get(&body.key) {
            if let Some(terrain) = &mut body.terrain {
                terrain.from_surface(source.clone());
            }
        }
    }
}
