use std::collections::HashSet;

use glam::Vec2;
pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

/// Per-frame input snapshot. "Pressed" sets are cleared after every frame.
#[derive(Default)]
pub struct Input {
    keys_down: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    buttons_down: HashSet<MouseButton>,
    buttons_pressed: HashSet<MouseButton>,
    /// Raw mouse motion accumulated this frame (device units, unaffected by cursor grab).
    pub mouse_delta: Vec2,
    /// Scroll wheel accumulated this frame, in "lines".
    pub scroll: f32,
}

impl Input {
    pub fn down(&self, key: KeyCode) -> bool {
        self.keys_down.contains(&key)
    }

    pub fn pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn button_down(&self, b: MouseButton) -> bool {
        self.buttons_down.contains(&b)
    }

    pub fn button_pressed(&self, b: MouseButton) -> bool {
        self.buttons_pressed.contains(&b)
    }

    /// +1 if `pos` is held, -1 if `neg` is held, 0 if both or neither.
    pub fn axis(&self, neg: KeyCode, pos: KeyCode) -> f32 {
        self.down(pos) as i32 as f32 - self.down(neg) as i32 as f32
    }

    pub(crate) fn key(&mut self, key: KeyCode, down: bool) {
        if down {
            if self.keys_down.insert(key) {
                self.keys_pressed.insert(key);
            }
        } else {
            self.keys_down.remove(&key);
        }
    }

    pub(crate) fn button(&mut self, b: MouseButton, down: bool) {
        if down {
            if self.buttons_down.insert(b) {
                self.buttons_pressed.insert(b);
            }
        } else {
            self.buttons_down.remove(&b);
        }
    }

    pub(crate) fn release_all(&mut self) {
        self.keys_down.clear();
        self.buttons_down.clear();
    }

    pub(crate) fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.buttons_pressed.clear();
        self.mouse_delta = Vec2::ZERO;
        self.scroll = 0.0;
    }
}
