//! One retained scene and one depth/light submission for structure + equipment.
use super::Craft;
use crate::equipment_visual::Camera;
use std::cell::RefCell;
use universe_engine::{
    Color, Frame, Light,
    glam::{DQuat, DVec3, Vec2},
    scene::{Node, ObjectId, Pose, Scene},
    viewport::PixelRect,
};

pub(super) fn draw(
    scene: &RefCell<Scene>,
    craft: &Craft,
    position: DVec3,
    rotation: DQuat,
    camera: &Camera,
    frame: &mut Frame,
) {
    if let Err(e) = submit(scene, craft, position, rotation, camera, frame) {
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
    let (clear, graphics) = (frame.clear, frame.graphics);
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
        child.clear = clear;
        child.graphics = graphics;
        child.studio = true;
        child.ambient = 0.3;
        child.light = Some(light);
    });
    Ok(())
}
