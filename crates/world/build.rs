//! The base world's fixed numbers (`content/base/sheet.ron`) into
//! `universe_world::sheet` (the kernel's laws are `universe_physics::laws`).

include!("../physics/build/sheetgen.rs");

fn main() {
    generate("../../content/base/sheet.ron", "sheet.rs", "universe_physics::sheet");
}
