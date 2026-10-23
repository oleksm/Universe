//! A hull's mechanisms (ramps, hatches, ladders, gear, doors) and its crew's exits, as its record declares them
//! (SFO hull `mechanisms`, `exits`, `model_sha256`), bound to its model's nodes. Discrete states only: each state
//! replaces its nodes' parent-local translation and rotation, then the node tree is evaluated once. No motion between
//! states, no sweep or load checks.
//!
//! Bound only to the exact model bytes the record names by hash: a replaced file under the same path is refused, so
//! poses never attach to geometry they weren't authored on.

use glam::{DMat4, DQuat, DVec3};
use sha2::{Digest, Sha256};

use crate::registry as r;

/// A node's scale may differ from 1 by this much (representation, not a design figure): beyond it, refused.
const UNIT_SCALE: f64 = 1e-4;
/// A rigid pose's quaternion norm may differ from 1 by this much (the schema's): beyond it, refused, not normalised.
const UNIT_QUAT: f64 = 1e-6;

/// The model's node tree: each node's exact name, parent and exported local transform.
#[derive(Clone, Debug, PartialEq)]
struct Node {
    name: String,
    parent: Option<usize>,
    local: DMat4,
}

/// A hull's mechanisms and exits, bound to its model.
#[derive(Clone, Debug, PartialEq)]
pub struct Bound {
    nodes: Vec<Node>,
    pub mechanisms: Vec<Mechanism>,
    pub exits: Vec<Exit>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mechanism {
    pub key: String,
    pub kind: r::MechanismKind,
    pub deploys_when: r::MechanismDeploysWhen,
    /// Its states' keys, in the record's order, and the default's index.
    pub states: Vec<String>,
    pub default: usize,
    /// The node that must meet the ground in the deployed state, where it has one.
    pub contact: Option<usize>,
    /// The nodes it owns.
    owned: Vec<usize>,
    /// Per state: each owned node's parent-local transform (the export's where `rest`).
    poses: Vec<Vec<(usize, DMat4)>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Exit {
    pub key: String,
    pub kind: r::ExitKind,
    /// (mechanism, state) indices, opened in this order, closed in reverse.
    pub requires: Vec<(usize, usize)>,
    pub top: usize,
    pub foot: Option<usize>,
}

/// The lower-case hex SHA-256 of a model's bytes.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Binds a hull record's mechanisms and exits to its model's bytes. None: it declares none.
pub fn bind(bytes: &[u8], hull: &r::Hull) -> Result<Option<Bound>, String> {
    if hull.mechanisms.is_empty() && hull.exits.is_empty() {
        return Ok(None);
    }
    let want = hull.model_sha256.as_deref().ok_or("mechanisms or exits without model_sha256")?;
    let have = sha256(bytes);
    if !want.eq_ignore_ascii_case(&have) {
        return Err(format!("model bytes are not the ones its mechanisms were authored on: sha256 {have}, record {want}"));
    }
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut nodes: Vec<Node> = g
        .nodes()
        .map(|n| Node { name: n.name().unwrap_or("").to_string(), parent: None, local: DMat4::from_cols_array_2d(&n.transform().matrix().map(|c| c.map(f64::from))) })
        .collect();
    for n in g.nodes() {
        for c in n.children() {
            nodes[c.index()].parent = Some(n.index());
        }
    }
    let find = |name: &str| -> Result<usize, String> {
        let mut it = nodes.iter().enumerate().filter(|(_, n)| n.name == name).map(|(i, _)| i);
        match (it.next(), it.next()) {
            (Some(i), None) => Ok(i),
            (None, _) => Err(format!("no node '{name}' in the model")),
            (Some(_), Some(_)) => Err(format!("node '{name}' is not unique in the model")),
        }
    };
    let unit_scale = |mut i: usize| -> Result<(), String> {
        loop {
            let (s, _, _) = nodes[i].local.to_scale_rotation_translation();
            if (s - DVec3::ONE).abs().max_element() > UNIT_SCALE {
                return Err(format!("node '{}' is scaled {s:?}: mechanism nodes and their parents must be unit scale", nodes[i].name));
            }
            match nodes[i].parent {
                Some(p) => i = p,
                None => return Ok(()),
            }
        }
    };
    let mut owner: std::collections::HashMap<usize, &str> = Default::default();
    let mut mechanisms = Vec::new();
    for m in &hull.mechanisms {
        let owned = m.nodes.iter().map(|n| find(n)).collect::<Result<Vec<_>, _>>()?;
        for &i in &owned {
            if let Some(other) = owner.insert(i, &m.key) {
                return Err(format!("node '{}' belongs to both {other} and {}", nodes[i].name, m.key));
            }
            unit_scale(i)?;
        }
        let mut poses = Vec::new();
        for s in &m.states {
            let mut pose = Vec::new();
            for p in &s.poses {
                let (node, local) = match p {
                    r::NodePose::Rest(x) => {
                        let i = find(&x.node)?;
                        (i, nodes[i].local)
                    }
                    r::NodePose::Rigid(x) => {
                        let [qx, qy, qz, qw] = x.rotation_xyzw;
                        let q = DQuat::from_xyzw(qx, qy, qz, qw);
                        let t = DVec3::from_array(x.translation_m);
                        if !(q.is_finite() && t.is_finite()) || (q.length() - 1.0).abs() > UNIT_QUAT {
                            return Err(format!("{} {}: {}'s pose is not a finite unit rotation and translation", m.key, s.key, x.node));
                        }
                        (find(&x.node)?, DMat4::from_rotation_translation(q, t))
                    }
                };
                if !owned.contains(&node) || pose.iter().any(|(n, _)| *n == node) {
                    return Err(format!("{} {}: '{}' posed but not owned, or posed twice", m.key, s.key, nodes[node].name));
                }
                pose.push((node, local));
            }
            if pose.len() != owned.len() {
                return Err(format!("{} {}: every node it owns needs one pose", m.key, s.key));
            }
            poses.push(pose);
        }
        let states: Vec<String> = m.states.iter().map(|s| s.key.clone()).collect();
        let default = states.iter().position(|s| *s == m.default).ok_or_else(|| format!("{}: no default state '{}'", m.key, m.default))?;
        let contact = m.contact.as_ref().map(|c| find(&c.node)).transpose()?;
        mechanisms.push(Mechanism { key: m.key.clone(), kind: m.kind, deploys_when: m.deploys_when, states, default, contact, owned, poses });
    }
    let mut exits = Vec::new();
    for e in &hull.exits {
        let requires = e
            .requires
            .iter()
            .map(|q| {
                let m = mechanisms.iter().position(|m| m.key == q.mechanism).ok_or_else(|| format!("exit {}: no mechanism {}", e.key, q.mechanism))?;
                let s = mechanisms[m].states.iter().position(|s| *s == q.state).ok_or_else(|| format!("exit {}: {} has no state {}", e.key, q.mechanism, q.state))?;
                Ok((m, s))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let foot = e.foot.as_deref().map(find).transpose()?;
        if foot.is_none() && e.kind != r::ExitKind::Walk {
            return Err(format!("exit {}: a {} exit needs its foot", e.key, if e.kind == r::ExitKind::Climb { "climb" } else { "step" }));
        }
        exits.push(Exit { key: e.key.clone(), kind: e.kind, requires, top: find(&e.top)?, foot });
    }
    Ok(Some(Bound { nodes, mechanisms, exits }))
}

impl Bound {
    /// Every mechanism at its default state.
    pub fn defaults(&self) -> Vec<usize> {
        self.mechanisms.iter().map(|m| m.default).collect()
    }

    /// Each node's model-space transform with the mechanisms in these states (one per mechanism): the states' local
    /// transforms set, then the tree evaluated once.
    fn world(&self, states: &[usize]) -> Vec<DMat4> {
        let mut local: Vec<DMat4> = self.nodes.iter().map(|n| n.local).collect();
        for (m, &s) in self.mechanisms.iter().zip(states) {
            for &(i, l) in &m.poses[s] {
                local[i] = l;
            }
        }
        let mut world: Vec<Option<DMat4>> = vec![None; self.nodes.len()];
        fn at(i: usize, nodes: &[Node], local: &[DMat4], world: &mut [Option<DMat4>]) -> DMat4 {
            if let Some(w) = world[i] {
                return w;
            }
            let w = nodes[i].parent.map_or(DMat4::IDENTITY, |p| at(p, nodes, local, world)) * local[i];
            world[i] = Some(w);
            w
        }
        (0..self.nodes.len()).map(|i| at(i, &self.nodes, &local, &mut world)).collect()
    }

    /// Each mechanism-owned node's rigid move in model space, from the export's pose to these states' (what hangs
    /// from a node moves with it).
    pub fn moves(&self, states: &[usize]) -> Vec<(String, DMat4)> {
        // (No states given: every node at its exported local transform, the export's own pose.)
        let (rest, now) = (self.world(&[]), self.world(states));
        self.mechanisms.iter().flat_map(|m| m.owned.iter()).map(|&i| (self.nodes[i].name.clone(), now[i] * rest[i].inverse())).collect()
    }

    /// A node's origin in model space with the mechanisms in these states.
    pub fn point(&self, node: usize, states: &[usize]) -> DVec3 {
        self.world(states)[node].transform_point3(DVec3::ZERO)
    }

    /// The node's exact name.
    pub fn name(&self, node: usize) -> &str {
        &self.nodes[node].name
    }

    /// An exit is open when every mechanism it requires is in the state it requires.
    pub fn open(&self, exit: &Exit, states: &[usize]) -> bool {
        exit.requires.iter().all(|&(m, s)| states[m] == s)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A glTF binary: one box mesh on the first node, the rest as the JSON gives them.
    pub(crate) fn glb(nodes: serde_json::Value, roots: &[usize]) -> Vec<u8> {
        let p: Vec<f32> = [[-2.0f32, -1.0, -3.0], [2.0, -1.0, -3.0], [2.0, 1.0, -3.0], [-2.0, 1.0, -3.0], [-2.0, -1.0, 3.0], [2.0, -1.0, 3.0], [2.0, 1.0, 3.0], [-2.0, 1.0, 3.0]].concat();
        let idx: [u16; 36] = [0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 6, 2, 3, 7, 6, 1, 2, 6, 1, 6, 5, 0, 4, 7, 0, 7, 3];
        let mut bin: Vec<u8> = p.iter().flat_map(|v| v.to_le_bytes()).collect();
        bin.extend(idx.iter().flat_map(|v| v.to_le_bytes()));
        let json = serde_json::json!({
            "asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": roots}], "nodes": nodes,
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 8, "type": "VEC3", "min": [-2, -1, -3], "max": [2, 1, 3]}, {"bufferView": 1, "componentType": 5123, "count": 36, "type": "SCALAR"}],
            "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 96}, {"buffer": 0, "byteOffset": 96, "byteLength": 72}],
            "buffers": [{"byteLength": bin.len()}]
        })
        .to_string();
        let mut j = json.into_bytes();
        while j.len() % 4 != 0 {
            j.push(b' ');
        }
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let mut out = Vec::new();
        out.extend(b"glTF");
        out.extend(2u32.to_le_bytes());
        out.extend(((12 + 8 + j.len() + 8 + bin.len()) as u32).to_le_bytes());
        out.extend((j.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(&j);
        out.extend((bin.len() as u32).to_le_bytes());
        out.extend(b"BIN\0");
        out.extend(&bin);
        out
    }

    /// A hull with a two-leaf ramp (a pivot, an extension on it, a toe on that) and its walk exit.
    fn ramp_hull(scale: f64) -> (Vec<u8>, r::Hull) {
        let bytes = glb(
            serde_json::json!([
                {"name": "Hull", "mesh": 0, "children": [1, 4]},
                {"name": "Ramp_Pivot", "translation": [0.0, -1.0, 3.0], "scale": [scale, scale, scale], "children": [2]},
                {"name": "Ramp_Ext", "translation": [0.0, 0.0, 2.0], "children": [3]},
                {"name": "Ramp_Toe", "translation": [0.0, 0.0, 2.0]},
                {"name": "Ramp_Top", "translation": [0.0, -1.0, 2.5]}
            ]),
            &[0],
        );
        // (Down by 90° about +X: the ramp's +Z along it turns to −Y, the floor's down.)
        let down = DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2);
        let mut hull = crate::registry::registry().hull("hull.mc-07").unwrap().clone();
        hull.model_sha256 = Some(sha256(&bytes));
        hull.mechanisms = serde_json::from_value(serde_json::json!([{
            "key": "rear_ramp", "kind": "ramp", "nodes": ["Ramp_Pivot", "Ramp_Ext"], "default": "closed", "deploys_when": "landed",
            "contact": {"node": "Ramp_Toe", "ground": "required"},
            "states": [
                {"key": "closed", "poses": [{"node": "Ramp_Pivot", "kind": "rest"}, {"node": "Ramp_Ext", "kind": "rest"}]},
                {"key": "deployed", "poses": [
                    {"node": "Ramp_Pivot", "kind": "rigid", "translation_m": [0.0, -1.0, 3.0], "rotation_xyzw": [down.x, down.y, down.z, down.w]},
                    {"node": "Ramp_Ext", "kind": "rest"}]}
            ]
        }]))
        .unwrap();
        hull.exits = serde_json::from_value(serde_json::json!([{"key": "rear", "kind": "walk", "requires": [{"mechanism": "rear_ramp", "state": "deployed"}], "top": "Ramp_Top"}])).unwrap();
        (bytes, hull)
    }

    /// The deployed state turns the pivot as recorded and carries what hangs from it; the exit opens with it; a
    /// different model under the record, a scaled pivot, or a missing node is refused.
    #[test]
    fn mechanisms_pose_by_record_and_bind_only_their_model() {
        let (bytes, hull) = ramp_hull(1.0);
        let b = bind(&bytes, &hull).unwrap().unwrap();
        let toe = b.mechanisms[0].contact.unwrap();
        let closed = b.defaults();
        assert!((b.point(toe, &closed) - DVec3::new(0.0, -1.0, 7.0)).length() < 1e-9);
        let deployed = [1];
        assert!((b.point(toe, &deployed) - DVec3::new(0.0, -5.0, 3.0)).length() < 1e-9, "{:?}", b.point(toe, &deployed));
        assert!(!b.open(&b.exits[0], &closed) && b.open(&b.exits[0], &deployed));
        let moves = b.moves(&deployed);
        assert_eq!(moves.len(), 2);
        assert!((moves[1].1.transform_point3(DVec3::new(0.0, -1.0, 7.0)) - DVec3::new(0.0, -5.0, 3.0)).length() < 1e-9);

        let mut other = bytes.clone();
        other.push(0);
        assert!(bind(&other, &hull).is_err_and(|e| e.contains("sha256")));
        let (scaled, mut hull2) = ramp_hull(1.5);
        hull2.model_sha256 = Some(sha256(&scaled));
        assert!(bind(&scaled, &hull2).is_err_and(|e| e.contains("unit scale")));
        let mut hull3 = hull.clone();
        hull3.exits[0].top = "Nowhere".into();
        assert!(bind(&bytes, &hull3).is_err_and(|e| e.contains("no node 'Nowhere'")));
    }
}
