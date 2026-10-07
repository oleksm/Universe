//! What a place does to what is there, by the registry's seeding record
//! (`seeding.conditions`): the heat a giant's kneading makes inside its
//! moons, and the radiation in the belt round a gas giant.

use glam::DVec3;

use crate::system::{BodyKind, StarSystem};
use crate::units::G;

fn record() -> Option<&'static crate::registry::Seeding> {
    crate::registry::registry().seeding("seeding.conditions")
}

/// The heat made inside body `i` (a moon) by its planet kneading it on its
/// stretched orbit (W): 21/2 (k2/Q) G M² R⁵ n e² / a⁶. None for anything else.
pub fn tidal_heat(sys: &StarSystem, i: usize) -> Option<f64> {
    let b = &sys.bodies[i];
    let parent = &sys.bodies[b.rail.parent?];
    if b.kind != BodyKind::Moon || !parent.kind.is_planet() {
        return None;
    }
    let k2q = record()?.tidal_heating.love_over_q?;
    let o = b.rail.orbit.as_ref()?;
    let (m, r, a, e) = (parent.mass, b.rail.radius, o.semi_major_axis, o.eccentricity);
    let n = (G * m / a.powi(3)).sqrt();
    Some(10.5 * k2q * G * m * m * r.powi(5) * n * e * e / a.powi(6))
}

/// The most a moon of radius `moon_radius` at `a` round a planet of `mass`
/// keeps stretched: its tides round its orbit off until they heat it no more
/// than the most active moon known (twice the record's volcanic threshold,
/// about Io's). Its orbit, as the seed makes it, is held to this.
pub fn tidal_eccentricity_limit(mass: f64, moon_radius: f64, a: f64) -> f64 {
    let Some(th) = record().map(|r| &r.tidal_heating) else { return f64::INFINITY };
    let (Some(k2q), Some(volcanic)) = (th.love_over_q, th.volcanic_above) else { return f64::INFINITY };
    let limit = 2.0 * volcanic;
    let n = (G * mass / a.powi(3)).sqrt();
    // (flux = 21/2 (k2/Q) G M² R⁵ n e² / a⁶ / (4π R²), for e.)
    (limit * 4.0 * std::f64::consts::PI * moon_radius * moon_radius * a.powi(6) / (10.5 * k2q * G * mass * mass * moon_radius.powi(5) * n)).sqrt()
}

/// That heat for each m² of its surface (W/m²).
pub fn tidal_flux(sys: &StarSystem, i: usize) -> Option<f64> {
    let r = sys.bodies[i].rail.radius;
    Some(tidal_heat(sys, i)? / (4.0 * std::f64::consts::PI * r * r))
}

/// What the tidal heat makes of a moon, by the registry's thresholds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kneaded {
    Slight,
    /// Warm inside: an icy one may keep a buried sea.
    BuriedSea,
    Volcanic,
    /// Far past any moon known: an orbit that couldn't last (it would have
    /// been made round long since).
    Impossible,
}

pub fn kneaded(sys: &StarSystem, i: usize) -> Option<Kneaded> {
    let flux = tidal_flux(sys, i)?;
    let th = &record()?.tidal_heating;
    let (volcanic, sea) = (th.volcanic_above.unwrap_or(1.0), th.sea_above.unwrap_or(0.03));
    Some(if flux >= 20.0 * volcanic { Kneaded::Impossible } else if flux >= volcanic { Kneaded::Volcanic } else if flux >= sea { Kneaded::BuriedSea } else { Kneaded::Slight })
}

/// The radiation dose at `p` (Sv/s) from the belts of the gas giants of
/// `sys` (their positions `positions`): as Jupiter's at the record's
/// distance, falling with distance as its power, in proportion to each
/// giant's mass.
pub fn radiation_dose(sys: &StarSystem, positions: &[DVec3], p: DVec3) -> f64 {
    let Some(rb) = record().map(|r| &r.radiation_belt) else { return 0.0 };
    let (Some(dose), Some(at), Some(falls), Some(jupiter)) = (rb.dose, rb.at, rb.falls_as, rb.giant_mass) else { return 0.0 };
    sys.bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BodyKind::GasGiant)
        .map(|(g, b)| {
            let radii = (positions[g].distance(p) / b.rail.radius).max(1.0);
            dose * (radii / at).powf(-falls) * b.mass / jupiter
        })
        .sum()
}

/// Whether a dose (Sv/s) kills an unshielded person within days, by the record.
pub fn deadly(dose: f64) -> bool {
    record().and_then(|r| r.radiation_belt.deadly_above).is_some_and(|d| dose >= d)
}
