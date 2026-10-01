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
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("universe").join("quicksave.json")
}

pub fn save(app: &mut App) -> Result<PathBuf, String> {
    let save = GameSave {
        universe: app.engine.call(|u| u.save()).ok_or("the world engine didn't answer")?,
        mode: app.mode,
        warp_index: app.warp_index,
        chase_cam: app.chase_cam,
        observer: app.observer.clone(),
    };
    let path = path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&save).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn load(app: &mut App) -> Result<(), String> {
    let json = std::fs::read_to_string(path()).map_err(|e| e.to_string())?;
    let save: GameSave = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let universe = save.universe;
    app.engine.call(move |u| u.load(universe)).ok_or("the world engine didn't answer")?;
    app.mode = save.mode;
    app.warp_index = save.warp_index.min(crate::WARPS.len() - 1);
    app.chase_cam = save.chase_cam;
    app.observer = save.observer;
    Ok(())
}
