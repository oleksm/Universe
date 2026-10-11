//! Every registry model reads through the game's loader (docs/asset-contract.md): each record that
//! carries `visual` has its package in the assets store, its manifest matching the record's hash, its
//! model matching the manifest, and bounds the game can centre it with. No assets store: nothing to read.

use universe_world::{assets, registry::registry};

#[test]
fn every_visual_reads() {
    if assets::store().is_none() {
        return;
    }
    let r = &registry().records;
    let keys: Vec<(&str, &universe_world::registry::Visual)> = r.equipment.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v)))
        .chain(r.modules.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v))))
        .chain(r.buildings.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v))))
        .chain(r.structures.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v))))
        .chain(r.gates.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v))))
        .chain(r.hulls.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v))))
        .chain(r.packages.iter().filter_map(|x| x.visual.as_ref().map(|v| (x.identity.key.as_str(), v))))
        .collect();
    let mut bad = Vec::new();
    for (key, v) in &keys {
        match assets::model(v) {
            Ok(m) if m.hi.iter().zip(&m.lo).all(|(h, l)| h > l) => println!("{key}: {} ({} bytes, bounds {:?} to {:?})", v.path, m.glb.len(), m.lo, m.hi),
            Ok(_) => bad.push(format!("{key}: {}: the manifest gives no bounds_m to centre it with", v.path)),
            Err(e) => bad.push(format!("{key}: {e}")),
        }
    }
    println!("{} model(s) read", keys.len() - bad.len());
    assert!(bad.is_empty(), "models the game cannot read:\n{}", bad.join("\n"));
}
