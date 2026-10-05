//! The base world's fixed numbers (`content/base/sheet.ron`) into
//! `universe_world::sheet` (Dogma's laws are `universe_physics::laws`), and
//! the registry (`standards/`) read, checked and encoded for the binary to
//! carry (`universe_world::registry`).

include!("../physics/build/sheetgen.rs");

fn main() {
    generate("../../content/base/sheet.ron", "sheet.rs", "universe_physics::sheet");
    println!("cargo:rerun-if-changed=../../standards");
    let reg = universe_registry::Registry::read(std::path::Path::new("../../standards")).unwrap_or_else(|problems| {
        for p in &problems {
            println!("cargo:warning={p}");
        }
        panic!("the registry has {} problem(s): the game isn't built from it until they're put right", problems.len());
    });
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("registry.bin");
    std::fs::write(dest, reg.encode()).unwrap();
}
