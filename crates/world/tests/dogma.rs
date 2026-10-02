//! The dogma's claims, checked against the physics sheet (`config/physics.ron`;
//! the charter: `docs/physics.md`). Change a number in the sheet and these say
//! which promise it breaks.

use universe_world::content::content;
use universe_world::modules::Does;
use universe_world::sheet::*;
use universe_world::units::{AU, LIGHT_YEAR};

/// The most power any plant makes, and the most per kg of plant.
fn best_plant() -> (f64, f64) {
    content().modules.iter().filter_map(|(_, m)| if let Does::PowerPlant { output } = m.does { Some((output, output / m.mass)) } else { None }).fold((0.0, 0.0), |(a, b), (o, d)| (a.max(o), b.max(d)))
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
}

#[test]
fn not_even_a_ship_all_reactor() {
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
}

#[test]
fn gates_are_justified_and_limited() {
    let power = |span_ly: f64| GATE_P0 * (span_ly / GATE_S0).powi(3);
    // A near lane is affordable infrastructure; the longest a ring can span is a giant's work.
    assert!(power(5.0) < 1e9 && power(RING_SPAN_III) > 1e11, "{:e} {:e}", power(5.0), power(RING_SPAN_III));
    assert!(RING_SPAN_I < RING_SPAN_II && RING_SPAN_II < RING_SPAN_III);
    // Freight by gate beats an explorer's crossing per kg (and any ship can take it).
    let gate = GATE_TAU * 40.0 * LIGHT_YEAR;
    let explorer = EXPLORER_POWER * 40.0 * LIGHT_YEAR / ((((EXPLORER_POWER * ETA_FIELD_MAX - P_FLOOR) / P_PUSH).cbrt()) * V_BEST_C * SPEED_OF_LIGHT);
    assert!(gate * 10.0 < explorer, "gate {gate:e} J/kg against an explorer's {explorer:e}");
}

#[test]
fn the_sheet_is_whole() {
    for e in SHEET {
        assert!(!e.note.is_empty(), "{} has no reason", e.name);
        assert!(e.value.is_finite(), "{}", e.name);
    }
}
