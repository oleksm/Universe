use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use universe_sim::UniverseSave;

use crate::observer::Observer;
use crate::{App, Mode};

#[derive(Serialize, Deserialize)]
struct GameSave {
    universe: UniverseSave,
    mode: Mode,
    warp_index: usize,
    chase_cam: bool,
    observer: Observer,
    /// The star systems we've been to.
    #[serde(default)]
    explored: Vec<usize>,
    /// Ship plans kept.
    #[serde(default)]
    plans: Vec<crate::shipyard::SavedPlan>,
    /// Hulls designed and commissioned, and the one on the board.
    #[serde(default)]
    designs: Vec<universe_sim::world::design::Design>,
    #[serde(default)]
    design: universe_sim::world::design::Design,
}

fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn path() -> PathBuf {
    data_dir().join("freefall").join("quicksave.json")
}

/// Where saves went before the game had its name (read if there's no newer one).
fn old_path() -> PathBuf {
    data_dir().join("universe").join("quicksave.json")
}

pub fn save(app: &mut App) -> Result<PathBuf, String> {
    let save = GameSave {
        universe: app.engine.save().ok_or("the world engine didn't answer")?,
        mode: app.mode,
        warp_index: app.warp_index,
        chase_cam: app.chase_cam,
        observer: app.observer.clone(),
        explored: app.explored.iter().copied().collect(),
        plans: app.plans.clone(),
        designs: app.designs.clone(),
        design: app.design.clone(),
    };
    let path = path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&save).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn load(app: &mut App) -> Result<(), String> {
    let json = std::fs::read_to_string(path()).or_else(|_| std::fs::read_to_string(old_path())).map_err(|e| e.to_string())?;
    let mut value: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    // The hulls designed first: a ship built to one is loaded as that hull.
    if let Some(list) = value.get("designs").cloned() {
        let designs: Vec<universe_sim::world::design::Design> = serde_json::from_value(list).unwrap_or_default();
        for d in &designs {
            let _ = d.commission();
        }
    }
    if let Some(u) = value.get_mut("universe") {
        universe_sim::save::forget_missing(u);
    }
    let save: GameSave = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let universe = save.universe;
    if universe.version < universe_sim::save::REGION_VERSION {
        return Err("saved in the old galaxy, before the charted region: its stars are gone".into());
    }
    // Made with other content (other packs, or another build's): keys keep
    // most of it, but say so.
    let other = universe.content != 0 && universe.content != universe_sim::world::content::content().hash();
    app.engine.load(universe).ok_or("the world engine didn't answer")?;
    if other {
        app.say("SAVED WITH OTHER CONTENT - WHAT'S GONE FROM IT IS LOST".into());
    }
    app.mode = save.mode;
    app.warp_index = save.warp_index.min(crate::WARPS.len() - 1);
    // (The cockpit view is gone: always the ship, from behind.)
    let _ = save.chase_cam;
    app.chase_cam = true;
    app.observer = save.observer;
    app.explored.extend(save.explored);
    app.plans = save.plans;
    app.designs = save.designs;
    app.design = save.design;
    Ok(())
}
