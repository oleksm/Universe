//! Climate: what the starlight does. Worlds' surface and air temperatures
//! (radiative equilibrium from their star, their albedo, a greenhouse from
//! their air, day and night swings damped by air, cooler toward the poles and
//! with height), and the light falling on a ship (its star's, in shadow none;
//! a near world's glow and reflection).

use glam::DVec3;
use crate::sheet::SOLAR_LUMINOSITY;
use universe_physics::laws::STEFAN_BOLTZMANN as SIGMA;

use crate::sheet::{ALBEDO_CRATERED, ALBEDO_DRY, ALBEDO_GIANT, ALBEDO_TERRAN, GREENHOUSE, LAPSE_RATE, NIGHT_FLOOR, SWING_DAMPING};
use crate::system::{BodyKind, StarSystem};
use crate::terrain::TerrainKind;

/// The system's star: (index, position).
fn star(sys: &StarSystem, positions: &[DVec3]) -> Option<(usize, DVec3)> {
    sys.bodies.iter().zip(positions).position(|(b, _)| b.kind == BodyKind::Star).map(|i| (i, positions[i]))
}

/// Starlight at `p` (W/m²), unshaded.
pub fn flux(sys: &StarSystem, positions: &[DVec3], p: DVec3) -> f64 {
    let Some((_, s)) = star(sys, positions) else { return 0.0 };
    sys.luminosity * SOLAR_LUMINOSITY / (4.0 * std::f64::consts::PI * p.distance_squared(s).max(1.0))
}

/// Is the star hidden from `p` by a world or moon?
pub fn in_shadow(sys: &StarSystem, positions: &[DVec3], p: DVec3) -> bool {
    let Some((si, s)) = star(sys, positions) else { return true };
    sys.bodies.iter().zip(positions).enumerate().any(|(i, (b, &c))| i != si && !b.kind.artificial() && universe_physics::segment_distance(p, s, c) < b.rail.radius)
}

/// A world's albedo.
pub fn albedo(sys: &StarSystem, i: usize) -> f64 {
    let b = &sys.bodies[i];
    match (b.kind, b.terrain.as_ref().map(|t| t.kind)) {
        (_, Some(TerrainKind::Terran)) => ALBEDO_TERRAN,
        (_, Some(TerrainKind::Dry)) => ALBEDO_DRY,
        (_, Some(TerrainKind::Cratered)) => ALBEDO_CRATERED,
        (BodyKind::GasGiant | BodyKind::IceGiant, _) => ALBEDO_GIANT,
        _ => ALBEDO_DRY,
    }
}

/// A world's mean surface temperature (K): its equilibrium with its star
/// (spread over its whole surface: it turns), plus its air's greenhouse.
pub fn mean_temperature(sys: &StarSystem, i: usize, positions: &[DVec3]) -> f64 {
    let s = flux(sys, positions, positions[i]);
    let equilibrium = (s * (1.0 - albedo(sys, i)) / (4.0 * SIGMA)).powf(0.25);
    equilibrium + greenhouse(sys, i)
}

fn greenhouse(sys: &StarSystem, i: usize) -> f64 {
    sys.bodies[i].rail.atmosphere.map_or(0.0, |a| GREENHOUSE * (a.surface_density / 1.225).powf(0.6))
}

/// A world's surface temperature (K) at `dir` (unit, from its centre, in the
/// system frame) at `t`: warm under the sun, cold at night (an airless world
/// to its bare extremes, air damping the swing), cooler toward its poles.
pub fn surface_temperature(sys: &StarSystem, i: usize, positions: &[DVec3], t: f64, dir: DVec3) -> f64 {
    let b = &sys.bodies[i];
    let Some((_, sun)) = star(sys, positions) else { return 3.0 };
    let s = flux(sys, positions, positions[i]);
    let a = albedo(sys, i);
    let mean = (s * (1.0 - a) / (4.0 * SIGMA)).powf(0.25);
    let to_sun = (sun - positions[i]).normalize_or_zero();
    let mu = dir.dot(to_sun);
    // Toward the poles (the spin axis), cooler.
    let axis = b.angular_velocity().try_normalize().unwrap_or(DVec3::Y);
    let _ = t;
    let lat = dir.dot(axis).abs();
    let by_latitude = mean * (1.0 - 0.25 * lat * lat);
    // The bare day (facing the sun, re-radiating what it takes in) and night.
    let day = (s * (1.0 - a) * mu.max(0.0) / SIGMA).powf(0.25);
    let night = NIGHT_FLOOR * mean;
    match b.rail.atmosphere {
        None => day.max(night * (1.0 - 0.3 * lat)),
        Some(air) => {
            let damp = 1.0 / (1.0 + air.surface_density / SWING_DAMPING);
            let swing = ((s * (1.0 - a) / SIGMA).powf(0.25) - night) * 0.5 * damp;
            by_latitude + greenhouse(sys, i) + swing * mu
        }
    }
}

/// The air's temperature at `p` over world `i` (K): its surface's under it,
/// cooling with height.
pub fn air_temperature(sys: &StarSystem, i: usize, positions: &[DVec3], t: f64, p: DVec3) -> f64 {
    let b = &sys.bodies[i];
    let off = p - positions[i];
    let altitude = off.length() - b.rail.radius;
    (surface_temperature(sys, i, positions, t, off.normalize_or_zero()) - LAPSE_RATE * altitude.max(0.0)).max(150.0)
}

/// What falls on a ship at `p` (W per m² it faces): the star's light (none
/// in a world's shadow), and from the nearest world, its glow (infrared, by
/// its mean temperature) and its reflected light, by how much of the sky it
/// fills.
pub fn light_at(sys: &StarSystem, positions: &[DVec3], p: DVec3) -> (f64, f64) {
    let sun = if in_shadow(sys, positions, p) { 0.0 } else { flux(sys, positions, p) };
    let near = (0..sys.bodies.len().min(positions.len()))
        .filter(|&i| !sys.bodies[i].kind.artificial() && sys.bodies[i].kind != BodyKind::Star)
        .min_by(|&a, &b| (positions[a].distance(p) - sys.bodies[a].rail.radius).total_cmp(&(positions[b].distance(p) - sys.bodies[b].rail.radius)));
    let world = near.map_or(0.0, |i| {
        let r = positions[i].distance(p).max(sys.bodies[i].rail.radius);
        let fill = (sys.bodies[i].rail.radius / r).powi(2);
        let glow = SIGMA * mean_temperature(sys, i, positions).powi(4);
        let lit = (p - positions[i]).normalize_or_zero().dot((star(sys, positions).map_or(p, |s| s.1) - positions[i]).normalize_or_zero()).max(0.0);
        fill * (glow + albedo(sys, i) * flux(sys, positions, positions[i]) * lit)
    });
    (sun, world)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Probe;

    #[test]
    fn an_earthlike_world_is_mild_an_airless_moon_swings_hard() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let pos = p.positions();
        let t = p.world.time;
        let sun = sys.bodies.iter().position(|b| b.kind == BodyKind::Star).unwrap();
        let to_sun = |i: usize| (pos[sun] - pos[i]).normalize();
        // An Earth-like world with air: its mean somewhere people live, day and night close.
        if let Some(w) = (0..sys.bodies.len()).find(|&i| sys.bodies[i].terrain.as_ref().is_some_and(|tr| tr.kind == TerrainKind::Terran)) {
            let mean = mean_temperature(&sys, w, &pos);
            let day = surface_temperature(&sys, w, &pos, t, to_sun(w));
            let night = surface_temperature(&sys, w, &pos, t, -to_sun(w));
            eprintln!("earth-like: mean {mean:.0} K, noon {day:.0} K, midnight {night:.0} K");
            assert!((200.0..340.0).contains(&mean), "{mean}");
            assert!(day > night && day - night < 60.0, "{day} {night}");
        }
        // An airless moon: a hot day, a deep cold night.
        let m = (0..sys.bodies.len()).find(|&i| sys.bodies[i].kind == BodyKind::Moon && sys.bodies[i].rail.atmosphere.is_none()).expect("a moon");
        let day = surface_temperature(&sys, m, &pos, t, to_sun(m));
        let night = surface_temperature(&sys, m, &pos, t, -to_sun(m));
        eprintln!("airless moon: noon {day:.0} K, midnight {night:.0} K");
        assert!(day - night > 100.0, "{day} {night}");
        // Air cools with height.
        let up = |i: usize, h: f64| pos[i] + to_sun(i) * (sys.bodies[i].rail.radius + h);
        if let Some(w) = (0..sys.bodies.len()).find(|&i| sys.bodies[i].rail.atmosphere.is_some()) {
            assert!(air_temperature(&sys, w, &pos, t, up(w, 5_000.0)) < air_temperature(&sys, w, &pos, t, up(w, 0.0)) - 20.0);
        }
    }
}
