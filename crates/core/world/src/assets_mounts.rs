//! Full glTF mount frames, kept separate from the shape's direction-only nodes.
use glam::{DMat4, DQuat, DVec3};
use std::collections::BTreeMap;

/// Named node frames in the default scene, including all ancestor transforms.
/// Duplicate names are refused rather than silently picking an attachment.
pub fn frames(bytes: &[u8]) -> Result<BTreeMap<String, DMat4>, String> {
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let scene = g
        .default_scene()
        .or_else(|| g.scenes().next())
        .ok_or("no glTF scene")?;
    fn visit(
        n: gltf::Node<'_>,
        parent: DMat4,
        out: &mut BTreeMap<String, DMat4>,
    ) -> Result<(), String> {
        let m =
            parent * DMat4::from_cols_array_2d(&n.transform().matrix().map(|c| c.map(f64::from)));
        if let Some(name) = n.name() {
            if out.insert(name.to_string(), m).is_some() {
                return Err(format!("duplicate glTF node name {name}"));
            }
        }
        for child in n.children() {
            visit(child, m, out)?;
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    for n in scene.nodes() {
        visit(n, DMat4::IDENTITY, &mut out)?;
    }
    Ok(out)
}

/// No invented look direction: preserve authored roll. Scale/shear/reflection
/// need a richer renderer contract and are explicitly refused, not discarded.
pub fn rigid(m: DMat4) -> Result<(DVec3, DQuat), String> {
    let [x, y, z, w] = m.to_cols_array_2d();
    if !m.is_finite() || [x[3], y[3], z[3], w[3] - 1.].iter().any(|v| v.abs() > 1e-6) {
        return Err("non-affine mount frame".into());
    }
    let axes = [
        m.x_axis.truncate(),
        m.y_axis.truncate(),
        m.z_axis.truncate(),
    ];
    if axes.iter().any(|a| (a.length_squared() - 1.).abs() > 2e-5)
        || axes[0].dot(axes[1]).abs() > 2e-5
        || axes[0].dot(axes[2]).abs() > 2e-5
        || axes[1].dot(axes[2]).abs() > 2e-5
        || (m.determinant() - 1.).abs() > 2e-5
    {
        return Err("mount scale, shear or reflection is unsupported".into());
    }
    Ok((m.w_axis.truncate(), DQuat::from_mat4(&m).normalize()))
}

/// Registry equipment mounting contract: exact mount_<slot>, then mount,
/// otherwise model-space identity. The GLB loader already applies its root.
pub fn placement(
    hull: DMat4,
    model: &BTreeMap<String, DMat4>,
    slot: &str,
) -> Result<DMat4, String> {
    rigid(hull)?;
    let primary = model
        .get(&format!("mount_{slot}"))
        .or_else(|| model.get("mount"))
        .copied()
        .unwrap_or(DMat4::IDENTITY);
    rigid(primary)?;
    let result = hull * primary.inverse();
    rigid(result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestor_frames_are_applied_once_and_duplicates_refused() {
        let bytes = br#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"translation":[1,2,3],"children":[1]},{"name":"mount","translation":[4,5,6]}]}"#;
        let f = frames(bytes).unwrap();
        assert_eq!(f["mount"].w_axis.truncate(), DVec3::new(5., 7., 9.));
        let duplicate=br#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0,1]}],"nodes":[{"name":"mount"},{"name":"mount"}]}"#;
        assert!(frames(duplicate).is_err());
    }
}

/// Refuse animated/skinned/morphed equipment in the static mount path.
pub fn require_static(bytes: &[u8]) -> Result<(), String> {
    let g = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    if g.animations().len() != 0
        || g.skins().len() != 0
        || g.meshes()
            .any(|m| m.primitives().any(|p| p.morph_targets().len() != 0))
    {
        return Err("animated, skinned or morphed equipment needs an articulated consumer".into());
    }
    Ok(())
}
