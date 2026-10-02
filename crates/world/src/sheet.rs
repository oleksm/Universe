//! The base world's fixed numbers: its designs, devices and technology as
//! they stand (a distro's, generated from `content/base/sheet.ron`). The
//! kernel's laws are `universe_physics::laws`.

include!(concat!(env!("OUT_DIR"), "/sheet.rs"));
