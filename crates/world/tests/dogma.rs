//! The dogma's claims: Dogma's laws (`config/dogma.ron`) checked against the
//! base world built on them (`content/base/`). The charter: `docs/physics.md`.
//! Change a law or the world, and these say which promise it breaks.

use universe_world::content::content;
use universe_world::modules::Does;
use universe_physics::laws::*;
use universe_world::sheet::*;
use universe_world::units::{AU, LIGHT_YEAR};

/// A field's efficiency, the best drive there is.
fn best_drive() -> f64 {
    content().modules.iter().filter_map(|(_, m)| if let Does::Hyperdrive { efficiency } = m.does { Some(efficiency) } else { None }).fold(0.0, f64::max)
}

/// How far a full tank takes a ship of `mass` kg carrying `fuel` kg of deuterium at `speed`
/// through a drive of efficiency `eta` (ly): its fuel's energy over the field's cost per metre.
fn range_ly(mass: f64, fuel: f64, speed: f64, eta: f64) -> f64 {
    let per_kg = universe_world::materials::material("material.deuterium").map(|m| m.energy).unwrap();
    fuel * per_kg / universe_physics::hyper::field_cost(mass, speed, eta) / LIGHT_YEAR
}

#[test]
fn a_normal_ship_cannot_reach_the_next_star_on_its_tank() {
    // Neighbours are 4-7 ly apart (docs/world/galaxy.md). Every hull on its own full tank, with
    // the best drive, going slow (its cheapest): short of 4 ly. Fuel's what holds it, nothing else.
    let v_star = V_BEST_C * SPEED_OF_LIGHT;
    for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.")) {
        let mass = h.dry_mass + h.fuel_capacity;
        let slow = range_ly(mass, h.fuel_capacity, 0.0, best_drive());
        assert!(slow < 4.0, "{} goes {slow:.1} ly on a tank", h.key);
        assert!(range_ly(mass, h.fuel_capacity, v_star, best_drive()) < slow * 0.51, "faster costs more");
    }
}

#[test]
fn an_explorer_that_is_mostly_tank_reaches_the_next_star() {
    // Nine tenths of it fuel, the best drive: about 5 ly at v*, in a couple of days.
    let v_star = V_BEST_C * SPEED_OF_LIGHT;
    let r = range_ly(1.0e5, 0.9e5, v_star, best_drive());
    assert!((4.0..8.0).contains(&r), "an explorer goes {r:.1} ly");
    let days = 5.0 * LIGHT_YEAR / v_star / 86_400.0;
    assert!((1.0..3.0).contains(&days), "5 ly takes {days:.1} days");
}

#[test]
fn hopping_round_a_system_costs_little() {
    // 40 AU at a third of v*: a sliver of a Drover's tank.
    let per_kg = universe_world::materials::material("material.deuterium").map(|m| m.energy).unwrap();
    let kg = universe_physics::hyper::field_cost(91_500.0, V_BEST_C * SPEED_OF_LIGHT / 3.0, 0.6) * 40.0 * AU / per_kg;
    assert!(kg < 30.0, "{kg:.1} kg for 40 AU");
    // Data across a relay's tube in a system: the flow's settle, a second or two.
    let hop = universe_world::hypernet::capsule_time(RELAY_CAPSULE, 2.0 * AU);
    assert!((1.0..2.5).contains(&hop), "a relay hop takes {hop} s");
}

#[test]
fn gates_are_justified_and_limited() {
    use universe_physics::hyper::{tube_crossing_energy, tube_hold_power, tube_natural_time, tube_open_energy};
    let s5 = 5.0 * LIGHT_YEAR;
    // Data at 200 ms a light year; a 100 t ship under a minute through a typical gate; a
    // capital ship several minutes (docs/world/hyperspace.md, tools/experiments/).
    assert!((tube_natural_time(GATE_CAPSULE, LIGHT_YEAR) - 0.2).abs() < 1e-9);
    let ship = tube_natural_time(1e5, s5);
    assert!((30.0..60.0).contains(&ship), "a 100 t ship takes {ship} s");
    assert!((180.0..900.0).contains(&tube_natural_time(1e8, s5)));
    // Rushing punishes: twice as fast costs e times as much; slow never free.
    let at = |t: f64| tube_crossing_energy(1e5, s5, t);
    assert!((at(ship / 2.0) / at(ship) - std::f64::consts::E).abs() < 1e-6);
    assert!(at(ship * 100.0) > at(ship) / std::f64::consts::E);
    // A pass at natural speed costs about an S2 plant-hour.
    assert!((1e10..1e11).contains(&at(ship)), "{:e}", at(ship));
    // Holding a gate is the economic choice: opening a one-ship tube for one pass costs
    // thousands of passes; a held gate pays at a handful of ships a day; opening it is a
    // faction's year.
    let gate = 2.0 * GATE_RADIUS;
    let own = tube_open_energy(100.0, s5);
    assert!(own > 1e3 * at(ship), "own tube {own:e} against a pass {:e}", at(ship));
    let day = tube_hold_power(gate, s5) * 86_400.0;
    assert!((1.0..50.0).contains(&(day / own)), "a held gate's day is {} own tubes", day / own);
    assert!((1e18..1e19).contains(&tube_open_energy(gate, s5)));
    // Relays' thin tubes are next to free to hold.
    assert!(tube_hold_power(RELAY_TUBE, 2.0 * AU) < 1.0);
    // Freight by gate beats a field's crossing per kg by far (and any ship can take it).
    let by_gate = tube_crossing_energy(1e5, 40.0 * LIGHT_YEAR, tube_natural_time(1e5, 40.0 * LIGHT_YEAR)) / 1e5;
    let by_field = universe_physics::hyper::field_cost(1e5, V_BEST_C * SPEED_OF_LIGHT, best_drive()) * 40.0 * LIGHT_YEAR / 1e5;
    assert!(by_gate * 1e6 < by_field, "gate {by_gate:e} J/kg against a field's {by_field:e}");
}

#[test]
fn the_sheets_are_whole() {
    for e in universe_physics::laws::SHEET.iter().chain(universe_world::sheet::SHEET) {
        assert!(!e.note.is_empty(), "{} has no reason", e.name);
        assert!(e.value.is_finite(), "{}", e.name);
    }
    // Dogma names no material (the laws are nature's and the hyper layer's: no fuels, no devices).
    for e in universe_physics::laws::SHEET {
        for word in ["FUEL", "DEUTERIUM", "HELIUM", "URANIUM", "METHALOX", "REACTOR", "CAPACITOR", "EXHAUST"] {
            assert!(!e.name.contains(word), "Dogma's laws name {}: that's the world's", e.name);
        }
    }
}
