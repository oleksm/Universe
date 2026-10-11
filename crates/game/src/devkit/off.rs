//! The developer's kit in a player build: the same names as `dev` and `observe`, doing nothing.

pub mod dev {
    use crate::App;

    pub fn world(seed: u64) -> Result<universe_sim::world::World, String> {
        if std::env::var_os("UNIVERSE_PGS1").is_some() {
            return Err("PGS1 preview requires a dev build".into());
        }
        Ok(universe_sim::world::World::new(seed))
    }

    /// No scenarios in a player build.
    pub fn apply(_app: &mut App, name: &str) {
        log::warn!("scenario {name:?}: not in this build (the dev feature is off)");
    }

    /// No sound test in a player build: it ends at once.
    pub fn sound_test(_ctx: &universe_engine::Context, _t: f64, _last_t: f64) -> bool {
        false
    }
}

pub mod observe {
    use crate::App;
    use universe_engine::Context;

    /// No live port in a player build: never asked anything.
    pub enum Request {}

    /// No recordings in a player build.
    pub struct Recording {
        pub dir: std::path::PathBuf,
    }

    impl Recording {
        pub fn seconds(&self) -> f64 {
            0.0
        }
    }

    pub fn clean_up() {}

    pub fn listen() -> Option<std::sync::mpsc::Receiver<Request>> {
        None
    }

    pub fn start(_app: &mut App, _ctx: &Context) {}

    pub fn frame(_app: &mut App, _ctx: &mut Context) {}

    pub fn stop(_app: &mut App, _ctx: &Context) {}

    pub fn answer(_app: &mut App, _ctx: &mut Context) {}
}
