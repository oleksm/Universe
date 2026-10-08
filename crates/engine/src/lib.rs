//! Universe engine: windowing, input, camera and a low-res retro vector renderer.
//!
//! World positions are `f64` (`DVec3`) so the engine can later span astronomical
//! distances. Everything is made camera-relative on the CPU before it reaches the
//! GPU, where `f32` is plenty.

pub mod app;
pub mod audio;
pub mod font;
pub mod camera;
mod env;
pub mod frame;
pub mod gpu;
pub mod input;
pub mod model;
pub mod pbr;
mod render_thread;
mod renderer;
mod sunprobe;
pub mod shaders;
pub mod devflags;
pub mod ground;
mod cloudcache;
pub mod gputime;
pub mod worldmaps;

pub use app::{run, Config, Context, Game, Perf, Resources, HISTORY, HITCH};
pub use camera::Camera;
pub use ground::{GroundFrame, GroundPass, GroundSetup};
pub use audio::{Audio, Jet};
pub use frame::{disc_covered, text_size, Color, Frame, Graphics, Light, Reflector, GLYPH};
pub use glam;
pub use input::{Input, KeyCode, MouseButton};
pub use model::{GlobeMap, Mesh, Transform, WireModel};
pub use pbr::PbrModel;
pub use sunprobe::sun_seen;
pub use worldmaps::WorldMaps;
