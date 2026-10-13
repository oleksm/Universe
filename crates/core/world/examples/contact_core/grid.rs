use super::Chart;
use crate::crease::{area, rect};
use glam::{DVec2, DVec3};
use universe_world::worlds::pgs::{Surface, Water};
#[derive(Clone)]
pub struct Triangle {
    pub tags: [[u32; 3]; 3],
    pub world: [DVec3; 3],
    pub uv: Vec<DVec2>,
    pub lo: DVec2,
    pub hi: DVec2,
}
pub fn visible(s: &Surface, d: DVec3) -> Result<f64, String> {
    let q = s.query(d)?;
    Ok(match q.water {
        Water::Wet { level_m, .. } => level_m,
        _ => q.height_m,
    })
}
fn face(f: u32, u: f64, v: f64) -> DVec3 {
    let (u, v) = (
        (u * std::f64::consts::FRAC_PI_4).tan(),
        (v * std::f64::consts::FRAC_PI_4).tan(),
    );
    match f {
        0 => DVec3::new(1., -v, -u),
        1 => DVec3::new(-1., -v, u),
        2 => DVec3::new(u, 1., v),
        3 => DVec3::new(u, -1., -v),
        4 => DVec3::new(u, -v, 1.),
        _ => DVec3::new(-u, -v, -1.),
    }
    .normalize()
}
fn cube(d: DVec3) -> u32 {
    let a = d.abs();
    if a.x >= a.y && a.x >= a.z {
        if d.x > 0. { 0 } else { 1 }
    } else if a.y >= a.z {
        if d.y > 0. { 2 } else { 3 }
    } else if d.z > 0. {
        4
    } else {
        5
    }
}
pub fn retained(s: &Surface, chart: &Chart) -> Result<(Vec<Triangle>, usize), String> {
    let f = cube(chart.c);
    for x in [-64., 64.] {
        for y in [-64., 64.] {
            if cube(chart.direction(DVec2::new(x, y))) != f {
                return Err("cube face boundary refusal".into());
            }
        }
    }
    let inverse = |d: DVec3| {
        let a = d.abs();
        let (u, v) = match f {
            0 => (-d.z / a.x, -d.y / a.x),
            1 => (d.z / a.x, -d.y / a.x),
            2 => (d.x / a.y, d.z / a.y),
            3 => (d.x / a.y, -d.z / a.y),
            4 => (d.x / a.z, -d.y / a.z),
            _ => (-d.x / a.z, -d.y / a.z),
        };
        DVec2::new(u.atan(), v.atan()) / std::f64::consts::FRAC_PI_4
    };
    let target = [(-64., -64.), (64., -64.), (64., 64.), (-64., 64.)]
        .map(|(x, y)| inverse(chart.direction(DVec2::new(x, y))));
    let target_lo = target
        .into_iter()
        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
    let target_hi = target
        .into_iter()
        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
    let mut stack = vec![[f, 0, 0, 0]];
    let mut out = vec![];
    let mut visits = 0;
    let eye = chart.c * (chart.r + visible(s, chart.c)? + 1.7);
    while let Some(key) = stack.pop() {
        visits += 1;
        if visits > 4096 {
            return Err("retained patch search cap".into());
        }
        let [f, l, x, y] = key;
        let n = (1u32 << l) as f64;
        let direction = |i: f64, j: f64| {
            face(
                f,
                (x as f64 + i) / n * 2. - 1.,
                (y as f64 + j) / n * 2. - 1.,
            )
        };
        let low = DVec2::new(x as f64, y as f64) / n * 2. - DVec2::ONE;
        let high = DVec2::new((x + 1) as f64, (y + 1) as f64) / n * 2. - DVec2::ONE;
        if high.x < target_lo.x
            || low.x > target_hi.x
            || high.y < target_lo.y
            || low.y > target_hi.y
        {
            continue;
        }
        let corners = [(0., 0.), (1., 0.), (1., 1.), (0., 1.)]
            .map(|(i, j)| chart.project(direction(i, j)))
            .into_iter()
            .collect::<Result<Vec<_>, _>>();
        if let Ok(ref corners) = corners {
            if area(&rect(corners.clone(), [-64., 64., -64., 64.])) < 1e-12 {
                continue;
            }
        }
        let mid = direction(0.5, 0.5);
        let size = std::f64::consts::FRAC_PI_2 / n * chart.r;
        let origin = mid * (chart.r + visible(s, mid)?);
        if l < 19 && (corners.is_err() || origin.distance(eye) < 2.4 * size) {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                stack.push([f, l + 1, x * 2 + dx, y * 2 + dy]);
            }
            continue;
        }
        let mut raw = Vec::with_capacity(289);
        for j in 0..17 {
            for i in 0..17 {
                let d = direction(i as f64 / 16., j as f64 / 16.);
                raw.push((d * (chart.r + visible(s, d)?) - origin).as_vec3());
            }
        }
        let at = |i: usize, j: usize| raw[j * 17 + i];
        let mut vertices = Vec::with_capacity(289);
        for j in 0..17 {
            for i in 0..17 {
                let own = at(i, j);
                let target = match (i % 2, j % 2) {
                    (0, 0) => own,
                    (1, 0) => (at(i - 1, j) + at(i + 1, j)) * 0.5,
                    (0, 1) => (at(i, j - 1) + at(i, j + 1)) * 0.5,
                    _ => (at(i - 1, j - 1) + at(i + 1, j + 1)) * 0.5,
                };
                let far = (2.4 * 2. * size * 0.95) as f32;
                let distance = (origin + own.as_dvec3() - eye).length() as f32;
                let t = ((distance - far * 0.6) / (far * 0.4)).clamp(0., 1.);
                vertices.push(origin + (own + (target - own) * (t * t * (3. - 2. * t))).as_dvec3());
            }
        }
        for j in 0..16 {
            for i in 0..16 {
                let a = j * 17 + i;
                for ids in [[a, a + 1, a + 18], [a, a + 18, a + 17]] {
                    let world = ids.map(|k| vertices[k]);
                    let uv = world
                        .map(|p| chart.project(p))
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?;
                    if area(&rect(uv.clone(), [-64., 64., -64., 64.])) < 1e-12 {
                        continue;
                    }
                    if out.len() >= 32768 {
                        return Err("retained triangle cap".into());
                    }
                    let lo = uv
                        .iter()
                        .copied()
                        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
                    let hi = uv
                        .iter()
                        .copied()
                        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
                    out.push(Triangle {
                        world,
                        uv,
                        lo,
                        hi,
                        tags: ids.map(|k| {
                            [
                                f,
                                (x * 16 + (k % 17) as u32) << (19 - l),
                                (y * 16 + (k / 17) as u32) << (19 - l),
                            ]
                        }),
                    });
                }
            }
        }
    }
    Ok((out, visits))
}
pub fn radial(t: &Triangle, d: DVec3) -> f64 {
    let [a, b, c] = t.world;
    let n = (b - a).cross(c - a);
    n.dot(a) / n.dot(d)
}
