//! What the renderer holds on the GPU, kind by kind, and what it may upload: one frame counter (a frame is
//! what `Renderer::render` draws, its views and all: a view is not a frame), one allowance a frame for each
//! kind's uploads, and one count of the bytes each kind holds (the memory report).
//!
//! The kinds keep their own ways of letting go (meshes unused a while, the pool's coldest ranges under
//! pressure, PBR models past their budget, globes' oldest layers, world maps one set at a time); they take
//! their uploads from here and report what they hold here.

/// What's held on the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Meshes uploaded one by one (`Mesh`).
    Meshes,
    /// The mesh pool's one buffer (`PooledMesh`).
    Pool,
    /// Textured, physically based models.
    Pbr,
    /// Worlds' globe maps (a fixed array of layers).
    Globes,
    /// The world near the eye's full maps.
    WorldMaps,
}

const KINDS: usize = 5;

impl Kind {
    fn at(self) -> usize {
        self as usize
    }

    /// Bytes this kind may upload a frame (its share of the copy bandwidth one frame can spare).
    fn allowance(self) -> usize {
        match self {
            // (Made where they're drawn: never held back, counted.)
            Kind::Meshes | Kind::Pool => usize::MAX,
            Kind::Pbr => 8 << 20,
            Kind::Globes => 2 << 20,
            Kind::WorldMaps => 2 << 20,
        }
    }
}

/// The renderer's residency: its frame counter, this frame's upload allowances, what each kind holds.
pub(crate) struct Residency {
    frame: u64,
    left: [usize; KINDS],
    held: [u64; KINDS],
}

impl Residency {
    pub fn new() -> Self {
        Self { frame: 0, left: [0; KINDS], held: [0; KINDS] }
    }

    /// A new frame (once for the frame and all its views): its number, and the allowances afresh.
    pub fn begin_frame(&mut self) -> u64 {
        self.frame += 1;
        for k in [Kind::Meshes, Kind::Pool, Kind::Pbr, Kind::Globes, Kind::WorldMaps] {
            self.left[k.at()] = k.allowance();
        }
        self.frame
    }

    /// The frame being drawn.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// What `kind` may still upload this frame (bytes).
    pub fn left(&self, kind: Kind) -> usize {
        self.left[kind.at()]
    }

    /// `bytes` of `kind` uploaded this frame.
    pub fn spend(&mut self, kind: Kind, bytes: usize) {
        let l = &mut self.left[kind.at()];
        *l = l.saturating_sub(bytes);
    }

    /// `kind` holds `bytes` on the GPU now.
    pub fn hold(&mut self, kind: Kind, bytes: u64) {
        self.held[kind.at()] = bytes;
    }

    pub fn held(&self, kind: Kind) -> u64 {
        self.held[kind.at()]
    }

    /// All the kinds hold together (bytes).
    pub fn total(&self) -> u64 {
        self.held.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame, its views and all: the allowance is spent across them, not given again to each.
    #[test]
    fn residency_allowance_is_a_frames_not_a_views() {
        let mut r = Residency::new();
        assert_eq!(r.begin_frame(), 1);
        r.spend(Kind::Globes, 1 << 20);
        r.spend(Kind::Globes, 1 << 20);
        assert_eq!(r.left(Kind::Globes), 0);
        assert_eq!(r.frame(), 1);
        assert_eq!(r.begin_frame(), 2);
        assert_eq!(r.left(Kind::Globes), 2 << 20);
        r.hold(Kind::Pbr, 5);
        r.hold(Kind::WorldMaps, 7);
        assert_eq!(r.total(), 12);
    }
}
