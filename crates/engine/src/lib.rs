//! Universe engine: windowing, input, camera and a low-res retro vector renderer.
//!
//! World positions are `f64` (`DVec3`) so the engine can later span astronomical
//! distances. Everything is made camera-relative on the CPU before it reaches the
//! GPU, where `f32` is plenty.

pub mod app;
pub mod audio;
pub mod camera;
pub mod frame;
pub mod gpu;
pub mod input;
pub mod model;
mod renderer;

pub use app::{run, Config, Context, Game, Perf};
pub use camera::Camera;
pub use audio::Audio;
pub use frame::{text_size, Color, Frame, Light, Reflector, GLYPH};
pub use glam;
pub use input::{Input, KeyCode, MouseButton};
pub use model::{Transform, WireModel};
