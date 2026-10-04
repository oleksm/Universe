//! Dogma's laws, from the registry (`standards/Dogma`), into `universe_physics::laws`.

include!("build/dogmagen.rs");

fn main() {
    generate_dogma("../../standards/Dogma/metadata", "laws.rs", "crate::sheet");
}
