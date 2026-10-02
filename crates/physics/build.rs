//! Dogma's laws (`config/dogma.ron`) into `universe_physics::laws`.

include!("build/sheetgen.rs");

fn main() {
    generate("../../config/dogma.ron", "laws.rs", "crate::sheet");
}
