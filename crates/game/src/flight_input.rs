//! Pilot commands shared by normal flight and the Studio test stand.
//! Cameras/cursor ownership and autopilot gates belong to each caller.
use universe_engine::{Input, KeyCode, glam::{DVec3, Vec2}};
use universe_sim::Controls;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pilot {
    pub turn: Controls,
    pub translation: DVec3,
    pub throttle_delta: f64,
}

pub fn sample(input: &Input, mouse: Option<Vec2>, dt: f64) -> Pilot {
    sample_keys(|key| input.down(key), mouse, dt)
}

fn sample_keys(down: impl Fn(KeyCode) -> bool, mouse: Option<Vec2>, dt: f64) -> Pilot {
    let axis = |neg, pos| f64::from(down(pos) as u8) - f64::from(down(neg) as u8);
    let shift = down(KeyCode::ShiftLeft) || down(KeyCode::ShiftRight);
    let keys = if shift { 0.0 } else { 1.0 };
    let mut turn = Controls {
        pitch: axis(KeyCode::ArrowUp, KeyCode::ArrowDown),
        yaw: axis(KeyCode::KeyE, KeyCode::KeyQ) * keys,
        roll: (axis(KeyCode::KeyD, KeyCode::KeyA) * keys + axis(KeyCode::ArrowRight, KeyCode::ArrowLeft)).clamp(-1.0, 1.0),
    };
    if let Some(m) = mouse {
        turn.pitch = (turn.pitch - f64::from(m.y) * 0.08).clamp(-1.0, 1.0);
        turn.yaw = (turn.yaw - f64::from(m.x) * 0.08).clamp(-1.0, 1.0);
    }
    Pilot {
        turn,
        translation: if shift { DVec3::new(axis(KeyCode::KeyA, KeyCode::KeyD), axis(KeyCode::KeyQ, KeyCode::KeyE), axis(KeyCode::KeyW, KeyCode::KeyS)) } else { DVec3::ZERO },
        throttle_delta: if shift { 0.0 } else { axis(KeyCode::KeyS, KeyCode::KeyW) * 0.6 * dt },
    }
}
