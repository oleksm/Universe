//! The charted world against the celestial registry (`content/base/celestial.ron`).

use universe_world::celestial::{apply_records, Stage};
use universe_world::content::content;
use universe_world::World;

/// The seed still makes the charted systems as the registry has them written
/// down: a change to how worlds are made that would change one fails here.
/// And a body the registry has taken over is as its record says.
#[test]
fn the_charted_world_is_as_the_registry_has_it() {
    let w = World::new(1984);
    let near = |a: f64, b: f64| (a - b).abs() <= 2e-3 * a.abs().max(b.abs());
    assert!(!content().celestial.is_empty(), "no systems written out");
    for rec in &content().celestial {
        let sys = w.system(rec.index);
        assert_eq!(sys.name, rec.system, "the star numbered {} has another name", rec.index);
        let natural = sys.bodies.iter().filter(|b| !b.kind.artificial() && b.rail.parent.is_some()).count();
        assert_eq!(natural, rec.bodies.len(), "{}: the seed makes {natural} bodies, the registry has {}", rec.system, rec.bodies.len());
        for r in &rec.bodies {
            let b = sys.bodies.iter().find(|b| b.name == r.name).unwrap_or_else(|| panic!("{}: the seed makes no {}", rec.system, r.name));
            let o = b.rail.orbit.as_ref();
            for (what, made, written) in [("mass", b.mass, r.mass), ("radius", b.rail.radius, r.radius), ("day", b.rail.day, r.day), ("orbit", o.map_or(0.0, |o| o.semi_major_axis), r.semi_major_axis.unwrap_or(0.0)), ("eccentricity", o.map_or(0.0, |o| o.eccentricity), r.eccentricity.unwrap_or(0.0))] {
                assert!(near(made, written), "{} ({}): its {what} is {made} in the game and {written} in the registry", r.name, r.status);
            }
        }
    }
    // (A record taken over stands in place of the seed's body, and its moons follow its new pull.)
    let rec = &content().celestial[0];
    let mut sys = universe_world::system::StarSystem::generate(rec.index, &w.galaxy.stars[rec.index]);
    let planet = rec.bodies.iter().find(|b| b.kind == "rocky planet").unwrap();
    let mut mine = planet.clone();
    mine.status = "curated".into();
    mine.mass *= 2.0;
    mine.radius *= 1.5;
    let i = sys.bodies.iter().position(|b| b.name == mine.name).unwrap();
    let before: Vec<(String, f64)> = sys.bodies.iter().filter(|m| m.rail.parent == Some(i)).map(|m| (m.name.clone(), m.rail.orbit.as_ref().unwrap().mu)).collect();
    apply_records(&mut sys, &[mine.clone()], Stage::Bodies, 0);
    assert_eq!(sys.bodies[i].mass, mine.mass);
    assert_eq!(sys.bodies[i].rail.radius, mine.radius);
    for (name, mu) in before {
        let m = sys.bodies.iter().find(|m| m.name == name).unwrap();
        assert!(near(m.rail.orbit.as_ref().unwrap().mu, 2.0 * mu), "{name} still goes round by the old pull");
    }
}
