// A world's air (the planet lab's, ported from its viewer's post pass in planet-sim's
// demo/index.html): single scattering along the view ray by molecules (Rayleigh) and aerosol
// (Mie, Cornette–Shanks phase), ozone absorbing; the sun's light on each point dimmed by the air
// on its way in, and none in the planet's shadow. Each world's coefficients from its bake's
// atmosphere.json (planet-sim: the climate and air stages), so a thin cold air, a hazy one, an
// ozone-less one each look as they should; no air at all, nothing drawn.
//
// Two entries, both in light units the caller's sun gives (`sun`: the sun's light, rgb, as the
// scene's lighting takes it: a white surface lit straight on, Lambert without the 1/π):
//   air_ground(c, p, center, sun_dir, sun, a) -> vec3   the ground's colour `c` at `p` seen through
//                                                        the air between it and the eye
//   air_sky(d, center, sun_dir, sun, a) -> vec3          the light the air sends along the view
//                                                        direction `d` from beyond everything drawn
// Positions camera-relative (m); `center` the world's centre, camera-relative (m). Worked in
// units of the world's radius about its centre (|p| ~ 1: 32 bits hold it to well under a metre).
//
// Status: draft on `planet`, validated with naga; waiting on the integrator's plumbing: `Air` as a
// uniform per world near the eye (from atmosphere.json), air_ground in fs_mesh in place of
// through_air, and a full-screen pass behind the scene calling air_sky.

struct Air {
    // Rayleigh scattering (= extinction) at the ground, per m (red, green, blue), and its scale height (m).
    beta_r: vec3<f32>,
    h_r: f32,
    // Mie scattering and extinction at the ground (per m), its scale height (m) and asymmetry g.
    mie_s: f32,
    mie_e: f32,
    h_m: f32,
    g: f32,
    // Ozone's absorption at its peak (per m, rgb), the peak's height and half-width (m): a tent.
    ozone: vec3<f32>,
    oz_peak: f32,
    oz_half: f32,
    // The air's top (m above the radius), the world's radius (m), and 1 if it has air at all.
    top_m: f32,
    radius_m: f32,
    on: f32,
};

const AIR_PI: f32 = 3.14159265;
// (The engine's light units: `sun` is the brightness of a white surface lit straight on, its
// Lambert term without the 1/π: irradiance / π. The march gives radiance per unit of irradiance,
// so its light is × π in those units.)
const AIR_UNITS: f32 = 3.14159265;
const AIR_STEPS: i32 = 32;
const AIR_SUN_STEPS: i32 = 8;

// Distances along o + t d to the sphere of radius r about the origin: (near, far); far < 0 if missed.
fn air_sphere(o: vec3<f32>, d: vec3<f32>, r: f32) -> vec2<f32> {
    let b = dot(o, d);
    let c = dot(o, o) - r * r;
    let h = b * b - c;
    if (h < 0.0) {
        return vec2<f32>(1e9, -1.0);
    }
    let s = sqrt(h);
    return vec2<f32>(-b - s, -b + s);
}

// Relative densities at a height (in radii): molecules, aerosol, ozone.
fn air_density(a: Air, hgt: f32) -> vec3<f32> {
    let h = max(hgt, 0.0) * a.radius_m;
    return vec3<f32>(exp(-h / a.h_r), exp(-h / a.h_m), max(0.0, 1.0 - abs(h - a.oz_peak) / a.oz_half));
}

// Extinction per radius at those densities.
fn air_extinction(a: Air, dn: vec3<f32>) -> vec3<f32> {
    return (a.beta_r * dn.x + vec3<f32>(a.mie_e) * dn.y + a.ozone * dn.z) * a.radius_m;
}

// The sun's light reaching p (radii, about the centre) through the air: its transmittance; zero in
// the planet's shadow.
fn air_sunlight(a: Air, p: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let ground = air_sphere(p, sun_dir, 1.0);
    if (ground.y > 0.0 && ground.x > 0.0) {
        return vec3<f32>(0.0);
    }
    let t1 = air_sphere(p, sun_dir, 1.0 + a.top_m / a.radius_m).y;
    let ds = max(t1, 0.0) / f32(AIR_SUN_STEPS);
    var tau = vec3<f32>(0.0);
    for (var i = 0; i < AIR_SUN_STEPS; i++) {
        let q = p + sun_dir * ds * (f32(i) + 0.5);
        tau += air_extinction(a, air_density(a, length(q) - 1.0)) * ds;
    }
    return exp(-tau);
}

// Along o + t d (radii) from t0 to t1: the transmittance (rgb) and the light scattered toward the
// eye per unit of sunlight (rgb).
struct AirPath {
    transmit: vec3<f32>,
    inscatter: vec3<f32>,
};

fn air_march(a: Air, o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32, sun_dir: vec3<f32>) -> AirPath {
    var out = AirPath(vec3<f32>(1.0), vec3<f32>(0.0));
    if (t1 <= t0) {
        return out;
    }
    let ds = (t1 - t0) / f32(AIR_STEPS);
    let mu = dot(d, sun_dir);
    let pr = 3.0 / (16.0 * AIR_PI) * (1.0 + mu * mu);
    let g2 = a.g * a.g;
    let pm = 3.0 / (8.0 * AIR_PI) * (1.0 - g2) * (1.0 + mu * mu) / ((2.0 + g2) * pow(1.0 + g2 - 2.0 * a.g * mu, 1.5));
    for (var i = 0; i < AIR_STEPS; i++) {
        let p = o + d * (t0 + ds * (f32(i) + 0.5));
        let dn = air_density(a, length(p) - 1.0);
        let ext = air_extinction(a, dn);
        let scat = (a.beta_r * dn.x * pr + vec3<f32>(a.mie_s) * dn.y * pm) * a.radius_m;
        let ts = exp(-ext * ds);
        // (Energy-conserving step: what scatters in over the step, dimmed within it.)
        out.inscatter += out.transmit * air_sunlight(a, p, sun_dir) * scat * (vec3<f32>(1.0) - ts) / max(ext, vec3<f32>(1e-9));
        out.transmit *= ts;
    }
    return out;
}

// The eye and a direction, about the world's centre in radii, and where the ray is in the air.
fn air_span(a: Air, eye: vec3<f32>, d: vec3<f32>, t_end: f32) -> vec2<f32> {
    let top = air_sphere(eye, d, 1.0 + a.top_m / a.radius_m);
    if (top.y <= 0.0) {
        return vec2<f32>(0.0, -1.0);
    }
    var t1 = min(top.y, t_end);
    // (The air stops at the ground, or the sea's surface: what lies under the water is seen
    // through the water, not more air.)
    let ground = air_sphere(eye, d, 1.0);
    if (ground.y > 0.0) {
        t1 = min(t1, max(ground.x, 0.0));
    }
    return vec2<f32>(max(top.x, 0.0), t1);
}

fn air_ground(c: vec3<f32>, p: vec3<f32>, center: vec3<f32>, sun_dir: vec3<f32>, sun: vec3<f32>, a: Air) -> vec3<f32> {
    if (a.on < 0.5) {
        return c;
    }
    let eye = -center / a.radius_m;
    let to = (p - center) / a.radius_m;
    let span = to - eye;
    let len = length(span);
    if (len < 1e-9) {
        return c;
    }
    let d = span / len;
    // (The point a hair inside the ground, so the march reaches it.)
    let t = air_span(a, eye, d, len * 1.0001);
    let path = air_march(a, eye, d, t.x, min(t.y, len), sun_dir);
    return c * path.transmit + path.inscatter * sun * AIR_UNITS;
}

fn air_sky(d: vec3<f32>, center: vec3<f32>, sun_dir: vec3<f32>, sun: vec3<f32>, a: Air) -> vec3<f32> {
    if (a.on < 0.5) {
        return vec3<f32>(0.0);
    }
    let eye = -center / a.radius_m;
    let t = air_span(a, eye, d, 1e9);
    let path = air_march(a, eye, d, t.x, t.y, sun_dir);
    return path.inscatter * sun * AIR_UNITS;
}
