use crate::yaml::pointer;
use crate::*;
use std::collections::BTreeSet;

/// Structural and local semantic errors. Does NOT claim registry records,
/// assets, dependency locks, hull anchors, structural loads or flight readiness
/// have been resolved; those require the caller's current dependencies.
pub fn validate(d: &Document) -> Vec<Diagnostic> {
    let mut out = crate::schema::structural(&serde_json::to_value(d).expect("document serializes"));
    let mut error = |path: String, message: &str| {
        out.push(Diagnostic {
            path,
            message: message.into(),
            location: None,
        })
    };
    // Optional floats serialize as null if non-finite; reject before they can
    // masquerade as omitted targets. Schema rejects null for all other numbers.
    if let Some(r) = &d.requirements {
        for (name, v) in [
            ("gravity_m_s2", r.gravity_m_s2),
            ("delta_v_m_s", r.delta_v_m_s),
            ("cargo_kg", r.cargo_kg),
            ("crew", r.crew),
            ("trip_s", r.trip_s),
        ] {
            if v.is_some_and(|v| !v.is_finite()) {
                error(format!("/requirements/{name}"), "number must be finite");
            }
        }
    }
    if let Some(v) = &d.view {
        if v.work_plane_m.is_some_and(|x| !x.is_finite()) {
            error("/view/work_plane_m".into(), "number must be finite");
        }
    }
    if let Some(h) = &d.hull {
        if !h.record.starts_with("hull.") {
            error("/hull/record".into(), "expected a hull registry key");
        }
    }
    let mut slots = BTreeSet::new();
    for (id, m) in &d.modules {
        let p = pointer("/modules", id);
        let pose = match m {
            Module::Equipment(e) => {
                if !e.record.starts_with("equipment.") {
                    error(format!("{p}/record"), "expected an equipment registry key");
                }
                if let Some(slot) = &e.slot {
                    if d.hull.is_none() {
                        error(format!("{p}/slot"), "slot binding requires a hull");
                    }
                    if !slots.insert(slot) {
                        error(
                            format!("{p}/slot"),
                            "hull slot is already bound by another module",
                        );
                    }
                }
                &e.pose
            }
            Module::HullHold(h) => {
                if d.hull.is_none() {
                    error(p.clone(), "hull_hold requires a hull");
                }
                &h.pose
            }
        };
        let norm = pose.rotation_xyzw.iter().fold(0.0f64, |n, x| n.hypot(*x));
        if !norm.is_finite() || (norm - 1.).abs() > 1e-6 {
            error(
                format!("{p}/pose/rotation_xyzw"),
                "rotation must be a unit quaternion (norm tolerance 1e-6); it is not normalized silently",
            );
        }
    }
    let mut occupied = BTreeSet::new();
    for (id, wire) in d.wiring.iter().flat_map(|w| w.iter()) {
        let p = pointer("/wiring", id);
        if !wire.cable.starts_with("run.") {
            error(format!("{p}/cable"), "expected a cable run registry key");
        }
        for (field, end) in [("from", &wire.from), ("to", &wire.to)] {
            if !matches!(d.modules.get(&end.module), Some(Module::Equipment(_))) {
                error(
                    format!("{p}/{field}/module"),
                    "wire endpoint must name an equipment instance",
                );
            }
            if !occupied.insert((&end.module, &end.port)) {
                error(
                    format!("{p}/{field}/port"),
                    "port is already occupied; use a junction with separate ports",
                );
            }
            if !end.connector.starts_with("connector.") {
                error(
                    format!("{p}/{field}/connector"),
                    "expected a connector registry key",
                );
            }
        }
        let mut conductors = BTreeSet::new();
        let mut from = BTreeSet::new();
        let mut to = BTreeSet::new();
        for (n, pin) in wire.pins.iter().enumerate() {
            if !conductors.insert(&pin.conductor)
                || !from.insert(&pin.from)
                || !to.insert(&pin.to)
            {
                error(
                    format!("{p}/pins/{n}"),
                    "conductor and endpoint contact assignments must be unique",
                );
            }
        }
        if wire
            .waypoints_m
            .windows(2)
            .any(|w| !nonzero_span(&w[0], &w[1]))
        {
            error(
                format!("{p}/waypoints_m"),
                "successive waypoints must have nonzero finite separation",
            );
        }
    }
    for (id, p) in &d.access.points {
        if p.hull_anchor.is_some() && d.hull.is_none() {
            error(
                format!("{}/hull_anchor", pointer("/access/points", id)),
                "hull anchor binding requires a hull",
            );
        }
    }
    for (id, p) in &d.access.paths {
        let path = pointer("/access/paths", id);
        for (field, key) in [("from", &p.from), ("to", &p.to)] {
            if !d.access.points.contains_key(key) {
                error(format!("{path}/{field}"), "dangling access point reference");
            }
        }
        if let (Some(a), Some(b)) = (d.access.points.get(&p.from), d.access.points.get(&p.to)) {
            if !nonzero_span(&a.position_m, &b.position_m) {
                error(path, "path must have nonzero finite length");
            }
        }
    }
    for (id, g) in &d.access.groups {
        let mut seen = BTreeSet::new();
        for (i, key) in g.paths.iter().enumerate() {
            let p = format!("{}/paths/{i}", pointer("/access/groups", id));
            if !d.access.paths.contains_key(key) {
                error(p.clone(), "dangling access path reference");
            }
            if !seen.insert(key) {
                error(p, "duplicate path in group");
            }
        }
    }
    for (id, door) in &d.access.side_doors {
        if !d.access.paths.contains_key(&door.path) {
            error(
                format!("{}/path", pointer("/access/side_doors", id)),
                "dangling access path reference",
            );
        }
    }
    let mut ends = BTreeSet::new();
    for (id, end) in &d.access.ends {
        let p = pointer("/access/ends", id);
        if !d.access.paths.contains_key(&end.path) {
            error(format!("{p}/path"), "dangling access path reference");
        }
        if !ends.insert((&end.path, end.end)) {
            error(p, "duplicate closure for this path end");
        }
    }
    for (id, m) in &d.structure.members {
        let p = pointer("/structure/members", id);
        if !m.stock.starts_with("stock.") {
            error(format!("{p}/stock"), "expected a stock registry key");
        }
        if !nonzero_span(&m.from_m, &m.to_m) {
            error(p, "member must have nonzero finite length");
        }
    }
    for (id, p) in &d.structure.plates {
        let path = pointer("/structure/plates", id);
        if !p.stock.starts_with("stock.") {
            error(format!("{path}/stock"), "expected a stock registry key");
        }
        if (0..2).any(|i| {
            !((p.max_xz_m[i] - p.min_xz_m[i]).is_finite() && p.max_xz_m[i] > p.min_xz_m[i])
        }) {
            error(
                path,
                "plate maximum must exceed minimum on both axes with finite extents",
            );
        }
    }
    for (id, deck) in &d.decks {
        let p = pointer("/decks", id);
        for (fid, poly) in &deck.floors {
            let path = pointer(&format!("{p}/floors"), fid);
            if poly.len() < 3 {
                error(path, "floor requires at least three vertices");
                continue;
            }
            let area: f64 = (0..poly.len())
                .map(|i| {
                    let a = poly[i];
                    let b = poly[(i + 1) % poly.len()];
                    a[0] * b[1] - a[1] * b[0]
                })
                .sum();
            if !area.is_finite() || area == 0. {
                error(
                    path.clone(),
                    "floor polygon must have nonzero finite signed area",
                );
            }
            if (0..poly.len()).any(|i| !nonzero_span(&poly[i], &poly[(i + 1) % poly.len()])) {
                error(
                    path,
                    "floor edges must have nonzero finite length (do not repeat closing vertex)",
                );
            }
        }
        for (wid, w) in &deck.walls {
            let path = pointer(&format!("{p}/walls"), wid);
            if w.bulges_m.len() != w.points_xz_m.len().saturating_sub(1) {
                error(
                    format!("{path}/bulges_m"),
                    "one bulge is required for every wall segment",
                );
            }
            if w.points_xz_m
                .windows(2)
                .any(|a| !nonzero_span(&a[0], &a[1]))
            {
                error(path, "wall segments must have nonzero finite length");
            }
        }
        for (lid, l) in &deck.ladders {
            let path = pointer(&format!("{p}/ladders"), lid);
            check_deck(d, id, &l.to_deck, &path, &mut error);
        }
        for (sid, s) in &deck.stairs {
            let path = pointer(&format!("{p}/stairs"), sid);
            check_deck(d, id, &s.to_deck, &path, &mut error);
            if !nonzero_span(&s.from_xz_m, &s.to_xz_m) {
                error(path, "stair must have nonzero finite horizontal run");
            }
        }
    }
    if let Some(dep) = &d.dependencies {
        if dep.policy == DependenciesPolicy::Locked && dep.registry_revision.is_none() {
            error(
                "/dependencies/registry_revision".into(),
                "locked dependencies require a registry revision; external lock coverage must also be resolved by the caller",
            );
        }
    }
    out.truncate(256);
    out
}
fn nonzero_span<const N: usize>(a: &[f64; N], b: &[f64; N]) -> bool {
    let n = (0..N).fold(0.0f64, |v, i| v.hypot(b[i] - a[i]));
    n.is_finite() && n > 0.
}
fn check_deck(
    d: &Document,
    from: &str,
    to: &str,
    path: &str,
    error: &mut impl FnMut(String, &str),
) {
    match d.decks.get(to) {
        None => error(
            format!("{path}/to_deck"),
            "dangling destination deck reference",
        ),
        Some(target) => {
            if from == to || d.decks[from].floor_m == target.floor_m {
                error(
                    format!("{path}/to_deck"),
                    "deck connection must reach a different elevation",
                );
            }
        }
    }
}
