//! Experimental shared client-side CPU terrain contact helpers.
//!
//! These are diagnostic/worker building blocks, not runtime-publishable geometry:
//! finite incident locking and excision remain pending. No GPU ownership is implied.

pub mod boundary_correction;
pub mod contact_core;
pub mod crease;
pub mod sparse_band;

pub use boundary_correction::retained::{
    Binding, BorrowedRetainedPatch, RetainedMesh, RetainedPatch, triangles_borrowed,
    triangles_retained,
};
pub use contact_core::Chart;

pub use sparse_band::{Prepared, build_prepared_typed};
