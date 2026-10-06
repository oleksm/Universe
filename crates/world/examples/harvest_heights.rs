//! Harvest's ground from its bake: its highest peak, and where its ports would stand.
use universe_world::worlds::{direction, Heights, LonLat};
fn main() {
    let t = std::time::Instant::now();
    let h = Heights::of("body.treistun.treistun-d").expect("Harvest's bake");
    println!("5 km heights read in {:.2} s; max {:.0} m", t.elapsed().as_secs_f64(), h.max);
    // The record's highest peak (8,920 m at 10.92 N, 110.96 W, in the bake's own longitude).
    for (name, p) in [("peak pk00001", LonLat { lat: 10.92, lon: -110.9564 }), ("Eikir (record)", LonLat { lat: 45.649, lon: 165.925 })] {
        let t = std::time::Instant::now();
        println!("{name}: {:.0} m ({:.3} s)", h.at(direction(p)), t.elapsed().as_secs_f64());
    }
}
