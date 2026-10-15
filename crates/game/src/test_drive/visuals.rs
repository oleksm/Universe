//! One retained scene and one depth/light submission for structure + equipment.
use super::Craft;
use crate::equipment_visual::Camera;
use std::cell::RefCell;
use universe_engine::{
    Color, Frame, Light,
    glam::{DQuat, DVec3, Vec2},
    scene::{Geometry, Node, ObjectId, Pose, Scene},
    viewport::PixelRect,
};

/// Laboratory marks are presentation geometry in the same depth buffer as the
/// craft. A HUD drawn before an opaque scene view would be covered by it.
#[derive(Clone, Copy)]
pub(super) enum Environment {
    Balance {
        floor: f64,
    },
    Flight {
        position: DVec3,
        time: f64,
        sky: Color,
    },
}

pub(super) fn draw(
    scene: &RefCell<Scene>,
    craft: &Craft,
    position: DVec3,
    rotation: DQuat,
    camera: &Camera,
    frame: &mut Frame,
    environment: Environment,
) {
    if let Err(e) = submit(scene, craft, position, rotation, camera, frame, environment) {
        frame.text_scaled(
            Vec2::new(16.0, frame.size().y - 36.0),
            &format!("SCENE: {e}"),
            Color([1.0, 0.4, 0.3, 1.0]),
            0.6,
        );
    }
}

fn submit(
    scene: &RefCell<Scene>,
    craft: &Craft,
    position: DVec3,
    rotation: DQuat,
    camera: &Camera,
    frame: &mut Frame,
    environment: Environment,
) -> Result<(), &'static str> {
    let view = camera.view(PixelRect {
        origin: Vec2::ZERO,
        size: frame.size(),
    })?;
    let mut scene = scene.borrow_mut();
    let root = ObjectId(0);
    let pose = Pose {
        position: position - rotation * craft.visual_origin.as_dvec3(),
        rotation,
        scale: 1.0,
    };
    if scene
        .get(root)
        .is_none_or(|n| n.pose.position != pose.position || n.pose.rotation != pose.rotation)
    {
        scene.set(
            root,
            Node {
                pose,
                ..Default::default()
            },
        )?;
    }
    for (i, placed) in craft.equipment.iter().chain(&craft.structure).enumerate() {
        placed.set_node(&mut scene, &ObjectId(i as u64 + 1), Some(root))?;
    }
    if let Some((path,centre)) = &craft.native {
        let id=ObjectId(u64::MAX);
        if scene.get(id).is_none() {
            if let Some(visual)=crate::visual_assets::request_native(std::path::Path::new(path),&[])
                .filter(|v|v.neutral_error.is_none()) {
                scene.set(id,Node {
                    parent:Some(root),pose:Pose {position:-*centre,..Default::default()},
                    geometry:Some(Geometry::Model {model:visual.model.with_lease(visual.clone()),part:0}),
                    ..Default::default()
                })?;
            }
        }
    }
    let graphics = frame.graphics;
    let light = Light {
        position: view.camera.position
            + (-camera.forward + camera.up * 0.7 - camera.right * 0.4)
                .normalize()
                .as_dvec3()
                * 10_000.0,
        color: [1.0; 3],
        luminosity: 1.0,
        reference: 10_000.0,
        radius: 100.0,
    };
    frame.scene_view(1, &scene, &view, |child| {
        child.clear = match environment {
            Environment::Balance { .. } => Color([0.03, 0.04, 0.08, 1.0]),
            Environment::Flight { sky, .. } => sky,
        };
        child.graphics = graphics;
        child.studio = true;
        child.ambient = 0.3;
        child.light = Some(light);
        environment.draw(child);
    });
    Ok(())
}

impl Environment {
    fn draw(self, frame: &mut Frame) {
        match self {
            Self::Balance { floor } => {
                for k in -8..=8 {
                    let o = f64::from(k) * 2.0;
                    let c = Color([0.3, 0.5, 0.4, 0.25]);
                    frame.line(DVec3::new(o, floor, -16.0), DVec3::new(o, floor, 16.0), c);
                    frame.line(DVec3::new(-16.0, floor, o), DVec3::new(16.0, floor, o), c);
                }
            }
            Self::Flight { position, time, .. } => {
                let reach = 150.0;
                let tile = 25.0;
                let cx = (position.x / tile).round() * tile;
                let cz = (position.z / tile).round() * tile;
                for i in -6..6 {
                    for j in -6..6 {
                        let x = cx + f64::from(i) * tile;
                        let z = cz + f64::from(j) * tile;
                        let far = DVec3::new(
                            x + tile * 0.5 - position.x,
                            0.0,
                            z + tile * 0.5 - position.z,
                        )
                        .length()
                            / reach;
                        let shade = if (i + j) % 2 == 0 { 0.16 } else { 0.12 };
                        let c = Color([0.2, 0.42, 0.32, shade * (1.0 - far).max(0.0) as f32]);
                        let [a, b, c0, d] = [
                            DVec3::new(x, 0.0, z),
                            DVec3::new(x + tile, 0.0, z),
                            DVec3::new(x + tile, 0.0, z + tile),
                            DVec3::new(x, 0.0, z + tile),
                        ];
                        frame.triangle(a, c0, b, c);
                        frame.triangle(a, d, c0, c);
                    }
                }
                // Lift ink slightly above the drawn plane to avoid depth fighting.
                let y = 0.015;
                let gx = (position.x / 5.0).round() * 5.0;
                let gz = (position.z / 5.0).round() * 5.0;
                let colour = |at: f64| {
                    if (at / 25.0).fract().abs() < 1e-3 {
                        Color([0.45, 0.75, 0.6, 0.55])
                    } else {
                        Color([0.35, 0.6, 0.5, 0.25])
                    }
                };
                for k in -30..=30 {
                    let o = f64::from(k) * 5.0;
                    frame.line(
                        DVec3::new(gx + o, y, gz - reach),
                        DVec3::new(gx + o, y, gz + reach),
                        colour(gx + o),
                    );
                    frame.line(
                        DVec3::new(gx - reach, y, gz + o),
                        DVec3::new(gx + reach, y, gz + o),
                        colour(gz + o),
                    );
                }
                let pad = DVec3::new(0.0, 0.03, -super::PAD_AHEAD);
                let half = super::PAD_ACROSS * 0.5;
                let gold = Color([1.0, 0.85, 0.35, 0.9]);
                frame.circle(pad, DVec3::Y, half, 48, gold);
                frame.circle(pad, DVec3::Y, half * 0.6, 48, Color([1.0, 0.85, 0.35, 0.5]));
                frame.line(
                    pad - DVec3::X * half * 0.4,
                    pad + DVec3::X * half * 0.4,
                    gold,
                );
                frame.line(
                    pad - DVec3::Z * half * 0.4,
                    pad + DVec3::Z * half * 0.4,
                    gold,
                );
                for k in 0..8 {
                    let a = f64::from(k) / 8.0 * std::f64::consts::TAU;
                    frame.point(
                        pad + DVec3::new(a.cos(), 0.0, a.sin()) * half,
                        Color([1.0, 0.4, 0.3, 1.0]),
                    );
                }
                frame.world_text(pad + DVec3::Y * 0.5, "PAD", gold, 0.6, true);
                let blink = if (time * 2.0).fract() < 0.5 {
                    1.0
                } else {
                    0.35
                };
                frame.line(DVec3::ZERO, DVec3::Y * 6.0, Color([0.8, 0.85, 0.9, 0.8]));
                frame.point(DVec3::Y * 6.0, Color([1.0, 0.3, 0.3, blink]));
                frame.world_text(
                    DVec3::Y * 6.5,
                    "BEACON",
                    Color([1.0, 0.5, 0.45, 0.9]),
                    0.6,
                    true,
                );
                frame.line(
                    position,
                    DVec3::new(position.x, y, position.z),
                    Color([0.6, 0.6, 0.6, 0.4]),
                );
            }
        }
    }
}
