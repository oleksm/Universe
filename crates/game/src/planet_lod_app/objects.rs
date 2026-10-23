//! Optional producer boxes: one shared cube, camera-relative instances, nearest visible labels.
use super::*;
use universe_terrain::package_colour;
use serde::Deserialize;
use universe_engine::glam::{DMat3, DQuat};

#[derive(Deserialize)]
struct Sidecar {
    legend: Vec<Legend>,
    boxes: Vec<RawBox>,
}
#[derive(Deserialize)]
struct Legend {
    family: String,
    rgb: [u8; 3],
}
#[derive(Deserialize)]
struct RawBox {
    lat: f64,
    lon: f64,
    z: f64,
    heading_deg: f64,
    length_m: f64,
    width_m: f64,
    height_m: f64,
    rgba: [f64; 4],
    #[serde(default)]
    label: String,
    family: String,
}
pub(super) struct Family {
    pub name: String,
    pub color: Color,
    pub on: bool,
}
struct Object {
    dir: DVec3,
    z: f64,
    size: Vec3,
    rotation: Quat,
    radius: f64,
    color: Color,
    label: String,
    family: usize,
}
pub(super) struct Objects {
    pub families: Vec<Family>,
    boxes: Vec<Object>,
    mesh: Mesh,
}
impl Objects {
    pub fn open(dir: &Path) -> Result<Option<Self>, String> {
        let path = dir.join("boxes.json");
        match std::fs::read_to_string(&path) {
            Ok(s) => Self::parse(&s)
                .map(Some)
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }
    fn parse(s: &str) -> Result<Self, String> {
        let data: Sidecar = serde_json::from_str(s).map_err(|e| e.to_string())?;
        let mut families: Vec<Family> = Vec::new();
        for l in data.legend {
            if l.family.is_empty() || families.iter().any(|f| f.name == l.family) {
                return Err("empty or duplicate legend family".into());
            }
            families.push(Family {
                name: l.family,
                color: Color(package_colour(l.rgb, 1.0)),
                on: true,
            });
        }
        let mut boxes = Vec::with_capacity(data.boxes.len());
        for (i, b) in data.boxes.into_iter().enumerate() {
            let fail = || format!("invalid box {i}");
            if ![
                b.lat,
                b.lon,
                b.z,
                b.heading_deg,
                b.length_m,
                b.width_m,
                b.height_m,
            ]
            .iter()
            .all(|v| v.is_finite())
                || b.lat.abs() > 90.0
                || b.lon.abs() > 180.0
                || [b.length_m, b.width_m, b.height_m]
                    .iter()
                    .any(|&v| v <= 0.0 || !(v as f32).is_finite())
                || b.rgba[..3]
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=255.0).contains(v))
                || !b.rgba[3].is_finite()
                || !(0.0..=1.0).contains(&b.rgba[3])
            {
                return Err(fail());
            }
            let family = families
                .iter()
                .position(|f| f.name == b.family)
                .ok_or_else(fail)?;
            let dir = unit(b.lon, b.lat);
            let lon = b.lon.to_radians();
            let east = DVec3::new(lon.cos(), 0.0, -lon.sin());
            let north = dir.cross(east);
            let (s, c) = b.heading_deg.to_radians().sin_cos();
            let rotation = DQuat::from_mat3(&DMat3::from_cols(
                east * c - north * s,
                dir,
                -(north * c + east * s),
            ))
            .as_quat();
            let size = Vec3::new(b.width_m as f32, b.height_m as f32, b.length_m as f32);
            let linear = |v: f64| {
                let x = (v / 255.0) as f32;
                if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            };
            boxes.push(Object {
                dir,
                z: b.z,
                size,
                rotation,
                radius: 0.5 * size.as_dvec3().length(),
                color: Color([
                    linear(b.rgba[0]),
                    linear(b.rgba[1]),
                    linear(b.rgba[2]),
                    b.rgba[3] as f32,
                ]),
                label: b.label,
                family,
            });
        }
        log::info!(
            "planet LOD: {} object boxes, {} families",
            boxes.len(),
            families.len()
        );
        let positions = (0..8)
            .map(|i| {
                Vec3::new(
                    if i & 1 == 0 { -0.5 } else { 0.5 },
                    if i & 2 == 0 { -0.5 } else { 0.5 },
                    if i & 4 == 0 { -0.5 } else { 0.5 },
                )
            })
            .collect();
        let faces = vec![
            [0, 4, 6],
            [0, 6, 2],
            [1, 3, 7],
            [1, 7, 5],
            [0, 1, 5],
            [0, 5, 4],
            [2, 6, 7],
            [2, 7, 3],
            [0, 2, 3],
            [0, 3, 1],
            [4, 5, 7],
            [4, 7, 6],
        ];
        let mesh = Mesh::new(WireModel {
            positions,
            faces,
            ..Default::default()
        });
        Ok(Self {
            families,
            boxes,
            mesh,
        })
    }
    pub fn select(&mut self, selection: &str) {
        for f in &mut self.families {
            f.on = selection == "all" || selection.split(',').any(|s| s == f.name);
        }
    }
    pub fn draw(&self, app: &PlanetLodApp, frame: &mut Frame) -> usize {
        let _scope = universe_prof::scope("draw/planetlod/objects");
        let cam = frame.camera;
        let size = frame.size();
        let aspect = (size.x / size.y.max(1.0)) as f64;
        let forward = (cam.orientation * Vec3::NEG_Z).as_dvec3();
        let right = (cam.orientation * Vec3::X).as_dvec3();
        let up = (cam.orientation * Vec3::Y).as_dvec3();
        let ty = (cam.fov_y as f64 * 0.5).tan();
        let tx = ty * aspect;
        let sx = (1.0 + tx * tx).sqrt();
        let sy = (1.0 + ty * ty).sqrt();
        let mut labels = Vec::new();
        let mut count = 0;
        for (i, b) in self.boxes.iter().enumerate() {
            if !self.families[b.family].on {
                continue;
            }
            let base = app.g.d.r + b.z * app.g.ex;
            let centre = b.dir * (base + 0.5 * b.size.y as f64);
            let to = centre - cam.position;
            let depth = to.dot(forward);
            if depth < -b.radius
                || to.dot(right).abs() > depth * tx + b.radius * sx
                || to.dot(up).abs() > depth * ty + b.radius * sy
            {
                continue;
            }
            if !app.g.above_horizon(
                b.dir,
                1.0 - b.radius * b.radius / (2.0 * base * base),
                b.z * app.g.ex + b.size.y as f64,
                cam.position,
            ) {
                continue;
            }
            frame.model_overlay_scaled(
                &self.mesh,
                &Transform {
                    position: centre,
                    rotation: b.rotation,
                    scale: 1.0,
                },
                b.size,
                b.color,
            );
            count += 1;
            if !b.label.is_empty() {
                let top = b.dir * (base + b.size.y as f64);
                if b.dir.dot(cam.position - top) > 0.0
                    && frame
                        .project(top)
                        .is_some_and(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y)
                {
                    labels.push(((top - cam.position).length_squared(), i, top));
                }
            }
        }
        nearest_labels(&mut labels);
        for (_, i, top) in labels {
            frame.world_text(top, &self.boxes[i].label, Color::WHITE, 0.85, true);
        }
        count
    }
}

fn nearest_labels(labels: &mut Vec<(f64, usize, DVec3)>) {
    let order =
        |a: &(f64, usize, DVec3), b: &(f64, usize, DVec3)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
    if labels.len() > 40 {
        labels.select_nth_unstable_by(40, order);
        labels.truncate(40);
    }
    labels.sort_unstable_by(order);
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &str = r#"{"legend":[{"family":"test","rgb":[255,128,0]}],"boxes":[{"lat":0,"lon":0,"z":12,"heading_deg":90,"length_m":100,"width_m":20,"height_m":30,"rgba":[255,128,0,0.35],"label":"test","family":"test"}]}"#;
    #[test]
    fn object_overlay_geometry_color_and_selection() {
        let mut o = Objects::parse(SAMPLE).unwrap();
        let b = &o.boxes[0];
        assert!((b.rotation * Vec3::NEG_Z - Vec3::X).length() < 1e-6);
        assert!((b.rotation * Vec3::Y - Vec3::Z).length() < 1e-6);
        assert_eq!(b.size, Vec3::new(20., 30., 100.));
        assert!((b.color.0[1] - 0.21586).abs() < 0.0001);
        assert_eq!(b.color.0[3], 0.35);
        o.select("none");
        assert!(!o.families[0].on);
        o.select("test");
        assert!(o.families[0].on);
        assert!(Objects::parse(&SAMPLE.replace("\"width_m\":20", "\"width_m\":0")).is_err());
        assert!(
            Objects::parse(&SAMPLE.replace("\"family\":\"test\"}", "\"family\":\"missing\"}"))
                .is_err()
        );
    }
    #[test]
    fn object_overlay_nearest_labels_are_capped() {
        let mut labels = (0..100).rev().map(|i| (i as f64, i, DVec3::ZERO)).collect();
        nearest_labels(&mut labels);
        assert_eq!(labels.len(), 40);
        assert_eq!(
            labels.iter().map(|x| x.1).collect::<Vec<_>>(),
            (0..40).collect::<Vec<_>>()
        );
    }
    #[test]
    #[ignore = "requires planet-forge sample view"]
    fn object_overlay_full_fixture() {
        let dir = crate::devenv::var("UNIVERSE_PLANETLOD_TEST_VIEW").unwrap();
        let o = Objects::open(Path::new(&dir)).unwrap().unwrap();
        assert_eq!(o.boxes.len(), 61179);
        assert_eq!(o.families.len(), 6);
    }
}
