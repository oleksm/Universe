//! The kernel's laws (`config/physics.ron`) into `universe_physics::laws`.

include!("build/sheetgen.rs");

fn main() {
    generate("../../config/physics.ron", "laws.rs", "crate::sheet");
}
