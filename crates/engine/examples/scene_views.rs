//! Bounded CPU/GPU acceptance harness, outside the owner's 50-test suite.
use universe_engine::pbr::{Material, PbrData, PbrModel, PbrVertex, Primitive};
use universe_engine::scene::{Geometry, Node, ObjectId, Pose, Scene, TickClock};
use universe_engine::viewport::{PixelRect, Projection, View};
use universe_engine::{
    Camera, Color, Config, Context, Frame, Game,
    glam::{DQuat, DVec3, Vec2, Vec3},
};
fn quad(z: f32, color: [f32; 4], blend: bool) -> PbrModel {
    PbrModel::new(PbrData {
        primitives: vec![Primitive {
            vertices: [[-1., -1., z], [1., -1., z], [1., 1., z], [-1., 1., z]]
                .map(|pos| PbrVertex {
                    pos,
                    normal: [0., 0., 1.],
                    tangent: [1., 0., 0., 1.],
                    uv: [(pos[0] + 1.) * 0.5, (pos[1] + 1.) * 0.5],
                })
                .to_vec(),
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            part: 0,
        }],
        materials: vec![Material {
            base_color: color,
            metallic: 0.,
            roughness: 1.,
            emissive: [0.; 3],
            normal_scale: 1.,
            occlusion_strength: 1.,
            alpha_cutoff: None,
            double_sided: true,
            blend,
            base_tex: None,
            mr_tex: None,
            normal_tex: None,
            emissive_tex: None,
            occlusion_tex: None,
        }],
        images: vec![],
    })
}
struct Fixture {
    scene: Scene,
    mask: bool,
}
fn checks() {
    let mut s = Scene::default();
    s.set(
        ObjectId(1),
        Node {
            pose: Pose {
                position: DVec3::new(2., 0., 0.),
                rotation: DQuat::from_rotation_z(0.4),
                scale: 2.,
            },
            ..Node::default()
        },
    )
    .unwrap();
    s.set(
        ObjectId(2),
        Node {
            parent: Some(ObjectId(1)),
            pose: Pose {
                position: DVec3::X,
                ..Pose::default()
            },
            bounds: Some([-DVec3::ONE, DVec3::ONE]),
            ..Node::default()
        },
    )
    .unwrap();
    let p = s.world_pose(ObjectId(2)).unwrap();
    assert!(
        (p.position - (DVec3::new(2., 0., 0.) + DQuat::from_rotation_z(0.4) * DVec3::X * 2.))
            .length()
            < 1e-12
    );
    assert!(
        s.set(
            ObjectId(1),
            Node {
                parent: Some(ObjectId(2)),
                ..Node::default()
            }
        )
        .is_err()
    );
    let ray = universe_engine::viewport::Ray {
        origin: p.position + DVec3::Z * 10.,
        direction: -DVec3::Z,
    };
    let hits = s.pick(ray, u64::MAX);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].0, ObjectId(2));
    assert!((hits[0].1 - 8.).abs() < 1e-12);
    let revision = s.revision();
    assert!(
        s.set(
            ObjectId(3),
            Node {
                parent: Some(ObjectId(99)),
                ..Node::default()
            }
        )
        .is_err()
    );
    assert_eq!(s.revision(), revision);
    for projection in [
        Projection::Perspective,
        Projection::Orthographic {
            height: 8.,
            far: 100.,
        },
    ] {
        let mut camera = Camera::default();
        camera.position = DVec3::new(1e10, 2e10, 3e10);
        camera.orientation = universe_engine::glam::Quat::from_rotation_y(0.3);
        let view = View {
            camera,
            projection,
            rect: PixelRect {
                origin: Vec2::new(50., 20.),
                size: Vec2::new(400., 300.),
            },
            layers: u64::MAX,
        };
        let point = camera.position + (camera.orientation * Vec3::new(0.5, 0.2, -5.)).as_dvec3();
        let screen = view.project(point).unwrap();
        let ray = view.ray(screen.truncate()).unwrap();
        assert!((point - ray.origin).cross(ray.direction).length() < 1e-4);
    }
    let mut clock = TickClock::new(0.25, 2).unwrap();
    let mut times = Vec::new();
    assert_eq!(clock.advance(1., |t, _| times.push(t)).unwrap(), 2);
    assert_eq!(clock.pending(), 0.5);
    clock.advance(0., |t, _| times.push(t)).unwrap();
    assert_eq!(times, vec![0., 0.25, 0.5, 0.75]);
    s.remove(ObjectId(1));
    assert!(s.get(ObjectId(2)).is_none());
    println!("scene transforms/cycles/removal, camera/ray roundtrip, scheduling: PASS");
}
impl Game for Fixture {
    fn update(&mut self, _: &mut Context) {}
    fn camera(&self) -> Camera {
        Camera::default()
    }
    fn draw(&self, f: &mut Frame, _: &Context) {
        let size = f.size();
        f.hud_rect(Vec2::ZERO, size, Color::rgb(0.05, 0.08, 0.15));
        for i in 0..3 {
            let mut camera = Camera::default();
            camera.position = DVec3::new(0., 0., 4.);
            if i == 1 {
                camera.position = DVec3::new(0., 0., -4.);
                camera.look_at(DVec3::ZERO, Vec3::Y);
            }
            if i == 2 {
                camera.position = DVec3::new(2., 1., 4.);
                camera.look_at(DVec3::ZERO, Vec3::Y);
            }
            if self.mask {
                camera.position = DVec3::new(if i == 1 { -4. } else { 4. }, 5., 6.);
                camera.look_at(DVec3::ZERO, Vec3::Y);
            }
            let view = View {
                camera,
                projection: if i == 2 {
                    Projection::Orthographic {
                        height: 4.,
                        far: 100.,
                    }
                } else {
                    Projection::Perspective
                },
                rect: PixelRect {
                    origin: Vec2::new(size.x * i as f32 / 3., size.y * 0.1),
                    size: Vec2::new(size.x / 3., size.y * 0.8),
                },
                layers: u64::MAX,
            };
            f.scene_view(i, &self.scene, &view, |child| {
                child.ambient = if self.mask { 0.05 } else { 1. };
                if self.mask {
                    child.light = Some(universe_engine::frame::Light {
                        position: DVec3::new(-10., 20., 10.),
                        color: [1.; 3],
                        luminosity: 1.,
                        reference: 30.,
                        radius: 0.,
                    });
                    child.shadow_reach = 120.;
                }
                child.studio = true;
                child.world_text(
                    DVec3::new(-0.7, 0.4, -0.5),
                    "HIDDEN",
                    Color::WHITE,
                    1.,
                    true,
                );
                child.world_text(
                    DVec3::new(-0.7, -0.4, 0.7),
                    "VISIBLE",
                    Color::WHITE,
                    1.,
                    true,
                );
            });
        }
        f.hud_rect(
            Vec2::new(0., size.y * 0.46),
            Vec2::new(size.x, 5.),
            Color::WHITE,
        );
    }
}
fn main() {
    checks();
    if std::env::args().any(|s| s == "--cpu") {
        return;
    }
    let mut scene = Scene::default();
    for (id, z, color, blend) in [
        (1, 0.3, [1., 0., 0., 0.5], true),
        (2, 0., [0., 1., 0., 0.5], true),
        (3, -0.3, [0., 0., 1., 1.], false),
    ] {
        scene
            .set(
                ObjectId(if std::env::var_os("REVERSE").is_some() {
                    4 - id
                } else {
                    id
                }),
                Node {
                    geometry: Some(Geometry::Model {
                        model: quad(z, color, blend),
                        part: 0,
                    }),
                    ..Node::default()
                },
            )
            .unwrap();
    }
    let mask = std::env::args().any(|s| s == "--mask");
    if mask {
        scene = mask_scene();
    }
    universe_engine::run(
        Config {
            title: "Shared scene views".into(),
            ..Default::default()
        },
        Fixture { scene, mask },
    );
}

fn mask_scene() -> Scene {
    let mut scene = Scene::default();
    let q = quad(0., [0.8, 0.8, 0.8, 1.], false);
    let mut data = std::sync::Arc::try_unwrap(q.data).unwrap();
    data.materials[0].alpha_cutoff = Some(if std::env::var_os("SOLID").is_some() {
        0.
    } else {
        0.5
    });
    data.materials[0].base_tex = Some(0);
    let rgba = (0..64)
        .flat_map(|i| {
            [
                255,
                255,
                255,
                if (i % 8 + i / 8) % 2 == 0 { 255 } else { 0 },
            ]
        })
        .collect();
    data.images.push(universe_engine::pbr::Image {
        width: 8,
        height: 8,
        rgba,
    });
    scene
        .set(
            ObjectId(1),
            Node {
                pose: Pose {
                    position: DVec3::Y,
                    rotation: DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2),
                    scale: 1.,
                },
                geometry: Some(Geometry::Model {
                    model: PbrModel::new(data),
                    part: 0,
                }),
                ..Node::default()
            },
        )
        .unwrap();
    scene
        .set(
            ObjectId(2),
            Node {
                pose: Pose {
                    rotation: DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2),
                    scale: 3.,
                    ..Pose::default()
                },
                geometry: Some(Geometry::Model {
                    model: quad(0., [0.8, 0.6, 0.3, 1.], false),
                    part: 0,
                }),
                ..Node::default()
            },
        )
        .unwrap();
    scene
}
