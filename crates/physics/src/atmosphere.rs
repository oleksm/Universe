//! Atmospheres: air around a body on rails, its density falling off
//! exponentially with height (an isothermal atmosphere), turning with the
//! body. A body moving through it is slowed by quadratic drag, and its
//! leading surfaces heat (Sutton–Graves stagnation-point heating).

use glam::DVec3;

use crate::rails::OnRails;

/// Sutton–Graves constant for air-like gases (kg^0.5 / m), with the heat flux
/// in W/m² from density in kg/m³, nose radius in m and speed in m/s.
pub use crate::laws::SUTTON_GRAVES;
use crate::laws::{AIR_TOP, EARTH_AIR_DENSITY, EARTH_SCALE_HEIGHT, STANDARD_GRAVITY};

/// Air around a body: density at the base radius (kg/m³), the height it
/// falls by a factor e over (m), and where it's taken to end (m above the
/// base radius).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    pub surface_density: f64,
    pub scale_height: f64,
    pub top: f64,
}

impl Atmosphere {
    /// Earth's: 1.225 kg/m³ at sea level, 8.5 km scale height. On a world
    /// with surface gravity `g` (m/s²) the same air stands taller or
    /// shorter in proportion (the scale height is kT/mg).
    pub fn earthlike(g: f64) -> Self {
        let scale_height = EARTH_SCALE_HEIGHT * STANDARD_GRAVITY / g.max(0.5);
        Atmosphere { surface_density: EARTH_AIR_DENSITY, scale_height, top: scale_height * AIR_TOP }
    }

    /// Density at `altitude` above the base radius (kg/m³); none above the top.
    pub fn density(&self, altitude: f64) -> f64 {
        if altitude >= self.top {
            0.0
        } else {
            self.surface_density * (-altitude.max(0.0) / self.scale_height).exp()
        }
    }
}

/// The air at `p`: its density and how it moves (turning with its body),
/// if `p` is inside an atmosphere. `t` is the time the bodies are at `positions`.
pub fn air_at<B: OnRails>(bodies: &[B], p: DVec3, positions: &[DVec3], t: f64) -> Option<(f64, DVec3)> {
    bodies.iter().zip(positions).enumerate().find_map(|(i, (b, &center))| {
        let r = b.rail();
        let air = r.atmosphere?;
        let off = p - center;
        let altitude = off.length() - r.radius;
        let density = air.density(altitude);
        (density > 0.0).then(|| (density, crate::rails::velocity(bodies, i, t) + r.angular_velocity().cross(off)))
    })
}

/// Quadratic drag over `h` seconds on a body moving at `velocity` through air
/// of `density` moving at `air`, for a ballistic coefficient `ballistic`
/// (mass / (drag coefficient × area), kg/m²). Exact for the drag alone
/// (dv/dt = -ρ|v|v / 2β along a fixed direction): stable at any step.
pub fn drag(velocity: DVec3, air: DVec3, density: f64, ballistic: f64, h: f64) -> DVec3 {
    let rel = velocity - air;
    let k = density / (2.0 * ballistic);
    air + rel / (1.0 + k * rel.length() * h)
}

/// Stagnation-point heat flux (W/m²) at `speed` through air of `density`, on
/// a nose of radius `nose` (m).
pub fn heat_flux(density: f64, speed: f64, nose: f64) -> f64 {
    SUTTON_GRAVES * (density / nose).sqrt() * speed.powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_falling_body_reaches_its_terminal_velocity() {
        // 1500 kg/m² in sea-level air under 9.81: v = sqrt(2βg/ρ) ≈ 155 m/s.
        let (beta, rho, g) = (1500.0, EARTH_AIR_DENSITY, STANDARD_GRAVITY);
        let mut v = DVec3::ZERO;
        for _ in 0..6000 {
            v += DVec3::NEG_Y * g * 0.05;
            v = drag(v, DVec3::ZERO, rho, beta, 0.05);
        }
        let terminal = (2.0 * beta * g / rho).sqrt();
        assert!((v.length() - terminal).abs() < 1.0, "{} vs {terminal}", v.length());
        // And a huge step is still stable.
        let fast = drag(DVec3::X * 8_000.0, DVec3::ZERO, rho, beta, 100.0);
        assert!(fast.x > 0.0 && fast.x < 8_000.0);
    }

}
