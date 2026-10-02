//! Writes the physics sheet's report, `docs/physics-sheet.md`: every constant
//! of `config/dogma.ron` with its unit, kind and reason, and what they add
//! up to. Run: `cargo run -p universe-world --example physics_sheet`.

use universe_world::content::content;
use universe_world::modules::Does;
use universe_physics::laws::*;
use universe_world::sheet::*;
use universe_world::units::{AU, LIGHT_YEAR};


fn main() {
    let mut md = String::from("# The physics sheet\n\nGenerated (run `cargo run -p universe-world --example physics_sheet`): edit the sheets, not this. The charter is `docs/physics.md`; the dogma's claims are checked in `crates/world/tests/dogma.rs`.\n\n");
    let mut table = |title: &str, source: &str, sections: &[(&str, &str)], sheet: &[universe_physics::sheet::Entry]| {
        md.push_str(&format!("# {title}\n\nFrom `{source}`.\n\n"));
        for &(section, note) in sections {
            md.push_str(&format!("## {section}\n\n{note}\n\n| Name | Value | Unit | Kind | Why |\n|---|---|---|---|---|\n"));
            for e in sheet.iter().filter(|e| e.section == section) {
                md.push_str(&format!("| `{}` | {} | {} | {:?} | {} |\n", e.name, fmt(e.value), e.unit, e.kind, e.note));
            }
            md.push('\n');
        }
    };
    table("Dogma's laws", "config/dogma.ron", universe_physics::laws::SECTIONS, universe_physics::laws::SHEET);
    table("The base world's numbers", "content/base/sheet.ron", universe_world::sheet::SECTIONS, universe_world::sheet::SHEET);
    md.push_str("# The base world's materials\n\nFrom `content/base/materials.ron`.\n\n| Material | Density (kg/m³) | Energy (J/kg) | Process | Trades as | Note |\n|---|---|---|---|---|---|\n");
    for (_, m) in content().materials.iter() {
        md.push_str(&format!("| {} | {} | {} | {:?} | {} | {} |\n", m.name, fmt(m.density), fmt(m.energy), m.process, if m.goods.is_empty() { "-" } else { &m.goods }, m.note));
    }
    md.push('\n');
    // What it adds up to.
    let (most, density) = content().modules.iter().filter_map(|(_, m)| if let Does::PowerPlant { output, .. } = m.does { Some((output, output / m.mass)) } else { None }).fold((0.0f64, 0.0f64), |(a, b), (o, d)| (a.max(o), b.max(d)));
    let wall = P_FLOOR / ETA_FIELD_MAX;
    md.push_str("## What it adds up to\n\n| Claim | Figure |\n|---|---|\n");
    md.push_str(&format!("| The wall between stars (holding the field, best drive) | {:.1} kW per kg aboard |\n", wall / 1000.0));
    md.push_str(&format!("| Today's best plant on its own | {:.1} kW/kg ({:.0} MW at most) |\n", density / 1000.0, most / 1e6));
    for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.")) {
        md.push_str(&format!("| {} with the strongest plant | {:.2} kW/kg: {} |\n", h.name, most / (h.dry_mass + h.fuel_capacity) / 1000.0, if most / (h.dry_mass + h.fuel_capacity) < wall { "can't cross" } else { "CAN CROSS" }));
    }
    let left = EXPLORER_POWER * ETA_FIELD_MAX - P_FLOOR;
    let k = (left.max(0.0) / P_PUSH).cbrt();
    let days = |ly: f64| ly * LIGHT_YEAR / (k * V_BEST_C * SPEED_OF_LIGHT) / 86_400.0;
    md.push_str(&format!("| A future explorer at {:.0} kW/kg | {:.2} × the best speed: 5 ly in {:.1} days, 40 ly in {:.1} days |\n", EXPLORER_POWER / 1000.0, k, days(5.0), days(40.0)));
    let slack = |d: f64| ((HYPER_RATE * d) / (V_OPEN_C * SPEED_OF_LIGHT)).powi(2).min(1.0);
    md.push_str(&format!("| The medium's slack at 1 AU / 40 AU / 2 ly | {:.0e} / {:.0e} / {} |\n", slack(AU), slack(40.0 * AU), slack(2.0 * LIGHT_YEAR)));
    use universe_physics::hyper::{tube_crossing_energy, tube_hold_power, tube_natural_time, tube_open_energy};
    let gate = 2.0 * universe_world::sheet::GATE_RADIUS;
    for span in [1.0, 5.0, 10.0, 40.0] {
        let s = span * LIGHT_YEAR;
        md.push_str(&format!("| A gate spanning {span} ly: opened at / held at | {:.1e} J / {} |\n", tube_open_energy(gate, s), watts(tube_hold_power(gate, s))));
    }
    for (what, m) in [("data (1 kg)", 1.0), ("a 100 t ship", 1e5), ("a capital ship (100 kt)", 1e8)] {
        let s = 5.0 * LIGHT_YEAR;
        let t = tube_natural_time(m, s);
        md.push_str(&format!("| {what} through a 5 ly gate at natural speed | {t:.1} s, {:.1e} J |\n", tube_crossing_energy(m, s, t)));
    }
    std::fs::write("docs/physics-sheet.md", md).expect("docs/physics-sheet.md (run from the repo's root)");
    println!("wrote docs/physics-sheet.md");
}

fn fmt(v: f64) -> String {
    if v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-3) { format!("{v:e}") } else { format!("{v}") }
}

fn watts(w: f64) -> String {
    if w >= 1e12 { format!("{:.1} TW", w / 1e12) } else if w >= 1e9 { format!("{:.1} GW", w / 1e9) } else { format!("{:.0} MW", w / 1e6) }
}
