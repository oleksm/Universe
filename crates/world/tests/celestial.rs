//! The charted world against the celestial registry (the seed's settings and
//! rock classes and the systems, all from `universe_world::registry`).

use universe_world::belt::RockClass;
use universe_world::celestial::{apply_records, systems, Stage};
use universe_world::registry::Provenance;
use universe_world::system::BodyKind;
use universe_world::registry::registry;
use universe_world::system::StarSystem;
use universe_world::World;

/// The seed still makes the charted systems as the registry has them written
/// down: a change to how worlds are made that would change one fails here.
/// And what the registry has taken over is as its record says.
#[test]
fn the_charted_world_is_as_the_registry_has_it() {
    let g = &registry().seeding.galaxy.galaxy;
    let w = World::new(g.seed);
    let home = registry().system(&g.home).expect("home is a system written out");
    assert_eq!(w.system(w.home_system).name, home.identity.name);
    // (The kinds of asteroid the seed makes are described: what the game needs of them is in their records.)
    use universe_world::belt::Structure;
    for key in ["rock-class.stony", "rock-class.carbonaceous", "rock-class.metallic", "rock-class.icy"] {
        let c = RockClass::by_key(key).unwrap_or_else(|| panic!("the registry has no {key}"));
        assert!(c.density(Structure::Rubble) > 0.0 && c.density(Structure::Monolith) > c.density(Structure::Rubble) && c.albedo() > 0.0, "{key}");
        let m = c.record().mining.as_ref().unwrap_or_else(|| panic!("{key} yields nothing"));
        assert!(m.cut_energy.is_some() && m.yields.is_some(), "{key}: how it's cut and what it yields");
        for good in m.yields.iter().chain(&m.rich_yields) {
            let ore = registry().good(good).and_then(|g| g.game.as_ref()?.ore.clone()).unwrap_or_else(|| panic!("{key} yields {good}, no ore of the game's"));
            assert!(universe_world::goods::Ore::from_key(&ore).is_some(), "{key}: the game has no ore {ore}");
        }
    }
    let near = |a: f64, b: f64| (a - b).abs() <= 2e-3 * a.abs().max(b.abs());
    assert!(!systems().is_empty(), "no systems written out");
    for rec in systems() {
        let sys = w.system(rec.index);
        assert_eq!(sys.name, rec.system, "the star numbered {} has another name", rec.index);
        assert_eq!(sys.class.letter().to_string(), rec.star.class);
        assert!(near(sys.luminosity, rec.star.luminosity), "{}: its star's luminosity", rec.system);
        let natural = sys.bodies.iter().filter(|b| !b.kind.artificial() && b.rail.parent.is_some()).count();
        assert_eq!(natural, rec.bodies.len(), "{}: the game has {natural} bodies, the registry {}", rec.system, rec.bodies.len());
        for r in &rec.bodies {
            let b = sys.bodies.iter().find(|b| b.name == r.name).unwrap_or_else(|| panic!("{}: the game has no {}", rec.system, r.name));
            let o = b.rail.orbit.as_ref();
            for (what, made, written) in [("mass", b.mass, r.mass), ("radius", b.rail.radius, r.radius), ("day", b.rail.day, r.day), ("orbit", o.map_or(0.0, |o| o.semi_major_axis), r.semi_major_axis.unwrap_or(0.0)), ("eccentricity", o.map_or(0.0, |o| o.eccentricity), r.eccentricity.unwrap_or(0.0))] {
                assert!(near(made, written), "{} ({:?}): its {what} is {made} in the game and {written} in the registry", r.name, r.status);
            }
        }
        assert_eq!(sys.fields.len(), rec.fields.len(), "{}: its fields", rec.system);
        for f in &rec.fields {
            let made = sys.fields.iter().find(|m| m.name == f.name).unwrap_or_else(|| panic!("{}: the game has no field {}", rec.system, f.name));
            assert!(made.count == f.count && near(made.extent, f.extent) && made.class(&sys) == f.class, "{}: the field differs", f.name);
        }
    }
    // What is taken over stands in place of the seed's. A planet: its mass and size as written, its
    // moons following its new pull, its orbit tilted as written.
    let rec = systems().iter().find(|s| s.system == home.identity.name).expect("home is written out");
    let fresh = || StarSystem::generate(rec.index, &w.galaxy.stars[rec.index]);
    let mut sys = fresh();
    let mut mine = rec.clone();
    let p = mine.bodies.iter().position(|b| b.kind == BodyKind::Rocky && rec.bodies.iter().any(|m| m.parent == b.name)).unwrap();
    let i = sys.bodies.iter().position(|b| b.name == mine.bodies[p].name).unwrap();
    let (mu, before): (f64, Vec<(String, f64)>) = (sys.bodies[i].rail.mu, sys.bodies.iter().filter(|m| m.rail.parent == Some(i)).map(|m| (m.name.clone(), m.rail.orbit.as_ref().unwrap().mu)).collect());
    mine.bodies[p].status = Provenance::Curated;
    mine.bodies[p].mass *= 2.0;
    mine.bodies[p].radius *= 1.5;
    mine.bodies[p].inclination = Some(0.2);
    assert!(apply_records(&mut sys, &mine, Stage::Bodies, 0).is_none());
    assert_eq!((sys.bodies[i].mass, sys.bodies[i].rail.radius), (mine.bodies[p].mass, mine.bodies[p].radius));
    assert!((sys.bodies[i].rail.orbit.as_ref().unwrap().inclination() - 0.2).abs() < 1e-9);
    for (name, was) in before {
        let m = sys.bodies.iter().find(|m| m.name == name).unwrap();
        assert!(near(m.rail.orbit.as_ref().unwrap().mu, was + sys.bodies[i].rail.mu - mu), "{name} still goes round by the old pull");
    }
    // A system: its star as written; a planet with no record gone, with its moons, and the rest
    // renumbered; one written that the seed doesn't make added.
    let mut sys = fresh();
    let mut mine = rec.clone();
    mine.status = Provenance::Curated;
    mine.star.mass *= 1.1;
    let gone = mine.bodies[p].name.clone();
    let mut added = mine.bodies[p].clone();
    added.name = "New World".into();
    added.semi_major_axis = added.semi_major_axis.map(|a| a * 1.01);
    mine.bodies.retain(|b| b.name != gone && b.parent != gone);
    mine.bodies.push(added);
    let count = sys.bodies.len();
    let moved = apply_records(&mut sys, &mine, Stage::Bodies, 0).expect("bodies were taken off");
    assert_eq!(moved.len(), count);
    assert!(sys.bodies.iter().all(|b| b.name != gone && b.rail.parent.is_none_or(|q| q < sys.bodies.len())));
    assert!(sys.bodies.iter().any(|b| b.name == "New World" && b.rail.parent == Some(0)));
    assert!(near(sys.bodies[0].mass, mine.star.mass * universe_world::units::SUN_MASS));
    // A field: its count, spread and class as written.
    let mut sys = fresh();
    let mut mine = rec.clone();
    mine.fields[0].status = Provenance::Curated;
    mine.fields[0].count += 7;
    let (icy, stony) = (RockClass::by_key("rock-class.icy").unwrap(), RockClass::by_key("rock-class.stony").unwrap());
    mine.fields[0].class = if mine.fields[0].class == icy { stony } else { icy };
    apply_records(&mut sys, &mine, Stage::Rocks, 0);
    let f = sys.fields.iter().find(|f| f.name == mine.fields[0].name).unwrap();
    assert!(f.count == mine.fields[0].count && f.class(&sys) == mine.fields[0].class);
}
