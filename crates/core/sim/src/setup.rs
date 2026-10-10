//! Where the world and its clients are put together: a universe with the
//! NPC pilots' pool and the player's cockpit, in step with it (the engine
//! hands them to their own threads later, see `engine`).

use crate::cockpit::Cockpit;
use crate::pilots::Pool;
use crate::universe::Universe;

impl Universe {
    /// A universe from `seed`, with its clients: the NPC pilots (none yet)
    /// and the player's cockpit, thinking in step with it.
    pub fn new(seed: u64) -> Self {
        Self::from_world(universe_world::World::new(seed))
    }

    /// Compose clients around an explicitly constructed world instance.
    pub fn from_world(world: universe_world::World) -> Self {
        let mut u = Universe::bare_world(world);
        u.npcs = Box::new(Pool::default());
        u.player = Some(Box::new(Cockpit::default()));
        u
    }
}
