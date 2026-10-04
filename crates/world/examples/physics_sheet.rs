//! Writes the physics sheet's report, `docs/physics-sheet.md`: every constant
//! of the Dogma registry (`standards/Dogma`) with its unit, kind and reason, and what they add
//! up to. Run: `cargo run -p universe-world --example physics_sheet`.

use universe_world::content::content;
use universe_physics::laws::*;
use universe_world::units::{DAY, LIGHT_YEAR};


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
    table("Dogma's laws", "standards/Dogma", universe_physics::laws::SECTIONS, universe_physics::laws::SHEET);
    table("The base world's numbers", "content/base/sheet.ron", universe_world::sheet::SECTIONS, universe_world::sheet::SHEET);
    md.push_str("# The base world's materials\n\nFrom `content/base/materials.ron`.\n\n| Material | Density (kg/m³) | Energy (J/kg) | Process | Trades as | Note |\n|---|---|---|---|---|---|\n");
    for (_, m) in content().materials.iter() {
        md.push_str(&format!("| {} | {} | {} | {:?} | {} | {} |\n", m.name, fmt(m.density), fmt(m.energy), m.process, if m.goods.is_empty() { "-" } else { &m.goods }, m.note));
    }
    md.push('\n');
    // What it adds up to.
    // A full tank in hyperdrive: fuel's energy over the field's cost per metre.
    let per_kg = universe_world::materials::material("material.deuterium").map_or(0.0, |m| m.energy);
    let v_star = V_BEST_C;
    md.push_str("## What it adds up to\n\n| Claim | Figure |\n|---|---|\n");
    for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.")) {
        let mass = h.dry_mass + h.fuel_capacity;
        let ly = |v: f64| h.fuel_capacity * per_kg / universe_physics::hyper::field_cost(mass, v, 0.6) / LIGHT_YEAR;
        md.push_str(&format!("| {} on a full tank (S drive, 0.6) | {:.1} ly slow, {:.1} ly at 1,000 c |\n", h.name, ly(0.0), ly(v_star)));
    }
    let explorer = 0.9e5 * per_kg / universe_physics::hyper::field_cost(1e5, v_star, 0.75) / LIGHT_YEAR;
    md.push_str(&format!("| An explorer, nine tenths tank (0.75) | {explorer:.1} ly at 1,000 c, 5 ly in {:.1} days |\n", 5.0 * LIGHT_YEAR / v_star / DAY));
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
