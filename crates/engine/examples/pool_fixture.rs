//! GPU fixture for the mesh pool: the same rolling ground, cut in clusters of 128 triangles, drawn
//! pooled (left) and as plain meshes (right): flat faces each its own colour, the grid's edges.
//! They should look the same. `POOL_FIXTURE_N=<clusters a side>` (default 16) for a load test,
//! `POOL_FIXTURE_ONLY=pool|mesh` to draw one side.
use universe_engine::glam::{DVec3, Quat, Vec3};
use universe_engine::{Camera, Color, Config, Context, Frame, Game, Light, Mesh, Morph, PoolModel, PooledMesh, Transform, WireModel};

const Q: usize = 8; // quads a cluster side: 128 triangles
const STEP: f32 = 1.0;

fn height(x: f32, y: f32) -> f32 {
    3.0 * (x * 0.21).sin() * (y * 0.17).cos() + 0.8 * (x * 0.9 + y * 0.6).sin()
}

fn shade(z: f32) -> [u8; 4] {
    let t = ((z + 4.0) / 8.0).clamp(0.0, 1.0);
    [(60.0 + 160.0 * t) as u8, (110.0 + 90.0 * t) as u8, (70.0 + 40.0 * (1.0 - t)) as u8, 255]
}

/// Cluster (cx, cy): its vertices relative to its corner, faces, edges, each face's colour.
fn cluster(cx: usize, cy: usize) -> PoolModel {
    let mut m = PoolModel::default();
    let (x0, y0) = ((cx * Q) as f32 * STEP, (cy * Q) as f32 * STEP);
    for j in 0..=Q {
        for i in 0..=Q {
            let (x, y) = (i as f32 * STEP, j as f32 * STEP);
            m.positions.push(Vec3::new(x, height(x0 + x, y0 + y), -y));
        }
    }
    let at = |i: usize, j: usize| (j * (Q + 1) + i) as u32;
    for j in 0..Q {
        for i in 0..Q {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            for f in [[a, b, c], [a, c, d]] {
                let z = f.iter().map(|&k| m.positions[k as usize].y).sum::<f32>() / 3.0;
                m.faces.push(f);
                m.face_colors.push(shade(z));
            }
            m.edges.push([a, b]);
            m.edges.push([a, d]);
        }
    }
    m
}

/// The same as a plain mesh: each face its own corners, coloured.
fn plain(p: &PoolModel) -> WireModel {
    let mut m = WireModel::default();
    for (f, c) in p.faces.iter().zip(&p.face_colors) {
        let k = m.positions.len() as u32;
        for &i in f {
            m.positions.push(p.positions[i as usize]);
            m.colors.push(c.map(|v| v as f32 / 255.0));
        }
        m.faces.push([k, k + 1, k + 2]);
    }
    for e in &p.edges {
        let k = m.positions.len() as u32;
        for &i in e {
            m.positions.push(p.positions[i as usize]);
            m.colors.push([1.0; 4]);
        }
        m.edges.push([k, k + 1]);
    }
    m
}

struct Fixture {
    n: usize,
    pooled: Vec<(PooledMesh, DVec3)>,
    plain: Vec<(Mesh, DVec3)>,
    only: Option<String>,
    frames: u64,
}

impl Game for Fixture {
    fn update(&mut self, ctx: &mut Context) {
        // (Where a frame's time goes, every two seconds or so: for the load test.)
        self.frames += 1;
        if self.frames % 240 == 0 {
            let p = &ctx.perf;
            let gpu: Vec<String> = universe_engine::gputime::times().iter().map(|(n, ms)| format!("{n} {ms:.2}")).collect();
            eprintln!("frame {:.2} ms: draw {:.2}, render {:.2}, wait {:.2}; GPU {}", p.frame_ms, p.draw_ms, p.render_ms, p.wait_ms, gpu.join(", "));
        }
    }
    fn camera(&self) -> Camera {
        let w = (self.n * Q) as f64 * STEP as f64;
        let mut c = Camera { position: DVec3::new(0.0, w * 0.55, w * 0.45), fov_y: 50f32.to_radians(), near: 0.5, ..Default::default() };
        c.look_at(DVec3::new(0.0, 0.0, -w * 0.45), Vec3::Y);
        c
    }
    fn draw(&self, f: &mut Frame, _: &Context) {
        f.light = Some(Light { position: DVec3::new(-1.0, 1.2, 0.6) * 1.5e11, color: [1.0, 0.98, 0.94], luminosity: 1.0, reference: 1.5e11, radius: 7e8 });
        f.clear = Color::rgb(0.02, 0.02, 0.05);
        let line = Color::hex(0xff6a00);
        if self.only.as_deref() != Some("mesh") {
            let g = f.pool_group(&Transform { position: self.pooled[0].1, rotation: Quat::IDENTITY, scale: 1.0 }, line, Color::WHITE, 1.0, true);
            for (m, o) in &self.pooled {
                f.pooled(g, m, *o);
            }
        }
        if self.only.as_deref() != Some("pool") {
            for (m, o) in &self.plain {
                f.model_colored_shaded(m, &Transform { position: *o, rotation: Quat::IDENTITY, scale: 1.0 }, 0.0, 1.0);
                f.model_shaded_faded(m, &Transform { position: *o, rotation: Quat::IDENTITY, scale: 1.0 }, line, Color([0.0; 4]), 1.0);
            }
        }
    }
}

fn main() {
    let n: usize = std::env::var("POOL_FIXTURE_N").ok().and_then(|v| v.parse().ok()).unwrap_or(16);
    let only = std::env::var("POOL_FIXTURE_ONLY").ok();
    let w = (n * Q) as f64 * STEP as f64;
    let (mut pooled, mut plain_meshes) = (Vec::new(), Vec::new());
    for cy in 0..n {
        for cx in 0..n {
            let mut m = cluster(cx, cy);
            // (POOL_FIXTURE_MORPH=1: the ground flattened with distance from the eye, by its morph
            // band, from 0.6 to 1.2 of its width away: the far half flat, the near rolling.)
            if std::env::var_os("POOL_FIXTURE_MORPH").is_some() {
                m.morph = m.positions.iter().map(|p| Morph { offset: Vec3::new(0.0, -p.y, 0.0), near: (w * 0.6) as f32, far: (w * 1.2) as f32 }).collect();
            }
            let corner = DVec3::new((cx * Q) as f64, 0.0, -((cy * Q) as f64));
            // (One side alone: in the middle, to compare the two at the same place.)
            let gap = w * 0.04;
            let (l, r) = if only.is_some() { (-w / 2.0, -w / 2.0) } else { (-w - gap, gap) };
            pooled.push((PooledMesh::new(&m), corner + DVec3::new(l, 0.0, 0.0)));
            plain_meshes.push((Mesh::new(plain(&m)), corner + DVec3::new(r, 0.0, 0.0)));
        }
    }
    let bytes: usize = pooled.iter().map(|(m, _)| m.bytes()).sum();
    eprintln!("{} clusters a side: pooled {} KB", n, bytes / 1024);
    universe_engine::run(Config { title: "Mesh pool fixture".into(), ..Default::default() }, Fixture { n, pooled, plain: plain_meshes, only, frames: 0 });
}
