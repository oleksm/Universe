//! The dogma's claims: Dogma's laws (`config/dogma.ron`) checked against the
//! base world built on them (`content/base/`). The charter: `docs/physics.md`.
//! Change a law or the world, and these say which promise it breaks.

use universe_world::content::content;
use universe_world::modules::Does;
use universe_physics::laws::*;
use universe_world::sheet::*;
use universe_world::units::{AU, LIGHT_YEAR};

/// The gate rings' spans (light years), by class: the world's ring products.
fn ring_spans() -> Vec<f64> {
    let mut spans: Vec<(u8, f64)> = content().structures.iter().filter_map(|(_, s)| if let universe_world::structures_catalogue::StructureKind::GateRing { class, span_ly } = s.kind { Some((class, span_ly)) } else { None }).collect();
    spans.sort_by_key(|s| s.0);
    spans.into_iter().map(|s| s.1).collect()
}

/// The most power any plant makes, and the most per kg of plant.
fn best_plant() -> (f64, f64) {
    content().modules.iter().filter_map(|(_, m)| if let Does::PowerPlant { output, .. } = m.does { Some((output, output / m.mass)) } else { None }).fold((0.0, 0.0), |(a, b), (o, d)| (a.max(o), b.max(d)))
}

/// Holding the field between stars needs at least this per kg aboard (the best drive).
fn wall() -> f64 {
    P_FLOOR / ETA_FIELD_MAX
}

#[test]
fn no_ship_today_can_cross_between_stars() {
    let (most, _) = best_plant();
    for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.")) {
        // (Generous: the strongest plant there is, in the lightest it can be — no cargo.)
        let per_kg = most / (h.dry_mass + h.fuel_capacity * 0.1);
        assert!(per_kg < wall(), "{} could hold a field between stars: {:.0} W/kg against the wall of {:.0}", h.key, per_kg, wall());
    }
    // Not even a ship that's all reactor.
    let (_, density) = best_plant();
    assert!(density < wall(), "a ship that's all reactor makes {density:.0} W/kg: past the wall of {:.0}", wall());
}

#[test]
fn a_future_explorer_makes_40_ly_an_epic() {
    // At its power per kg, after holding the field, what's left pushes: P_PUSH·k³ = EXPLORER_POWER·η − P_FLOOR.
    let left = EXPLORER_POWER * ETA_FIELD_MAX - P_FLOOR;
    assert!(left > 0.0, "the reference explorer can't hold the field");
    let k = (left / P_PUSH).cbrt();
    let days = |ly: f64| ly * LIGHT_YEAR / (k * V_BEST_C * SPEED_OF_LIGHT) / 86_400.0;
    assert!((0.5..3.0).contains(&days(5.0)), "5 ly takes {:.1} days", days(5.0));
    assert!((5.0..30.0).contains(&days(40.0)), "40 ly takes {:.1} days: an epic, not a trip nor a lifetime", days(40.0));
}

#[test]
fn within_a_system_the_medium_is_stiff() {
    // The slack at 1 AU and at 40 AU from a star (no other mass near): next to nothing.
    let slack = |d: f64| ((HYPER_RATE * d) / (V_OPEN_C * SPEED_OF_LIGHT)).powi(2).min(1.0);
    assert!(slack(AU) < 1e-5, "{}", slack(AU));
    assert!(slack(40.0 * AU) < STIFF_SLACK, "a field must still form at the outer planets: {}", slack(40.0 * AU));
    // And between stars it's all slack.
    assert!(slack(2.0 * LIGHT_YEAR) >= 1.0);
    // A hyper-signal: next to free across a system, dear between stars.
    use universe_physics::hyper::bit_energy;
    assert!(bit_energy(slack(AU), AU, 0.6) < 1e-6, "{}", bit_energy(slack(AU), AU, 0.6));
    assert!(bit_energy(1.0, 5.0 * LIGHT_YEAR, 0.6) > 1.0, "{}", bit_energy(1.0, 5.0 * LIGHT_YEAR, 0.6));
    // A signal through a throat in microseconds; matter in its 10 s.
    assert!(universe_physics::hyper::throat_signal_time() < 1e-3);
}

#[test]
fn gates_are_justified_and_limited() {
    let power = |span_ly: f64| GATE_P0 * (span_ly / GATE_S0).powi(3);
    // A near lane is affordable infrastructure; the longest a ring can span is a giant's work.
    let spans = ring_spans();
    let longest = *spans.last().expect("the world builds gate rings");
    assert!(power(5.0) < 1e9 && power(longest) > 1e11, "{:e} {:e}", power(5.0), power(longest));
    assert!(spans.windows(2).all(|w| w[0] < w[1]), "a higher class spans farther: {spans:?}");
    // Freight by gate beats an explorer's crossing per kg (and any ship can take it).
    let gate = GATE_TAU * 40.0 * LIGHT_YEAR;
    let explorer = EXPLORER_POWER * 40.0 * LIGHT_YEAR / ((((EXPLORER_POWER * ETA_FIELD_MAX - P_FLOOR) / P_PUSH).cbrt()) * V_BEST_C * SPEED_OF_LIGHT);
    assert!(gate * 10.0 < explorer, "gate {gate:e} J/kg against an explorer's {explorer:e}");
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
