//! The explicit operations that move a body other than by integration. Each is
//! a plain, auditable function with the same rules for every body.

use glam::{DQuat, DVec3};

use crate::body::RigidBody;
use crate::collide::Contact;
use crate::rails::{velocity, Frame, OnRails};

/// A body held fixed in a rail body's frame (resting on it, or attached to
/// it), carried round with its orbit and spin.
#[derive(Clone, Copy, Debug)]
pub struct Weld {
    /// Index of the rail body.
    pub body: usize,
    /// In the rail body's rotating frame.
    pub local_position: DVec3,
    pub local_orientation: DQuat,
}

impl Weld {
    /// Weld `rb` to rail body `body` where it is at `t` (`positions` at `t`).
    pub fn capture<B: OnRails>(bodies: &[B], body: usize, t: f64, positions: &[DVec3], rb: &RigidBody) -> Self {
        let rot = bodies[body].rail().rotation(t).inverse();
        Self { body, local_position: rot * (rb.position - positions[body]), local_orientation: rot * rb.orientation }
    }

    /// Put `rb` where the weld holds it at `t`, moving with the rail body's
    /// material there.
    pub fn place<B: OnRails>(&self, bodies: &[B], t: f64, positions: &[DVec3], rb: &mut RigidBody) {
        let b = bodies[self.body].rail();
        let rot = b.rotation(t);
        let offset = rot * self.local_position;
        rb.position = positions[self.body] + offset;
        rb.velocity = velocity(bodies, self.body, t) + b.angular_velocity().cross(offset);
        rb.orientation = rot * self.local_orientation;
    }
}

/// A body's pose and motion relative to a frame (ignoring the frame's spin),
/// to be carried over to another frame.
#[derive(Clone, Copy, Debug)]
pub struct Relative {
    pub position: DVec3,
    pub velocity: DVec3,
    pub orientation: DQuat,
}

impl Relative {
    pub fn of(frame: &Frame, rb: &RigidBody) -> Self {
        let back = frame.rotation.inverse();
        Self { position: frame.local(rb.position), velocity: back * (rb.velocity - frame.velocity), orientation: back * rb.orientation }
    }

    /// Put `rb` at this pose and motion relative to `frame`.
    pub fn place(&self, frame: &Frame, rb: &mut RigidBody) {
        rb.position = frame.center + frame.rotation * self.position;
        rb.velocity = frame.velocity + frame.rotation * self.velocity;
        rb.orientation = frame.rotation * self.orientation;
    }
}

/// Move `rb` from one frame to another, keeping its pose and motion relative to it.
pub fn relocate(from: &Frame, to: &Frame, rb: &mut RigidBody) {
    Relative::of(from, rb).place(to, rb);
}

/// Bounce off a contact: the velocity into the surface is reversed and scaled
/// by `restitution`, plus a separating `separation` (m/s) along the normal, and
/// the body is pushed `push` meters clear.
pub fn bounce(rb: &mut RigidBody, contact: &Contact, restitution: f64, separation: f64, push: f64) {
    let (n, rel) = (contact.normal, contact.relative_velocity);
    let into = rel.dot(n).min(0.0);
    rb.velocity = contact.surface_velocity + rel - n * ((1.0 + restitution) * into) + n * separation;
    rb.position += n * push;
}
