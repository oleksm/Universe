//! Observing the running game, for whoever's helping debug it.
//!
//! **Observing** (SHIFT+F3, again to stop): the live port answering, the
//! profiler and the hitch log on, and a recording started.
//!
//! **Recordings** (with observing, or `/record/start`): the profiler and a trace on,
//! every frame captured, for up to `MAX_SECONDS`; then a session folder —
//! frames as PNGs, every frame's time and scopes (`profile.json`), a timeline
//! of every thread (`trace.json`, Chrome's format: Perfetto opens it), the
//! game's state at the start and end, and a summary to read first. The newest
//! `KEEP` sessions are kept; older ones, and any over a day old, are cleared
//! at start-up.
//!
//! **The live port** (127.0.0.1 only, `UNIVERSE_OBSERVE_PORT`, default 7878;
//! `UNIVERSE_OBSERVE=0`: none; opened the first time observing comes on,
//! answering only while observing): plain HTTP, for the now — where we are, the
//! frame times, the profile, what the renderer holds, the graphics settings
//! (set them too), a screenshot, recordings started and stopped. `GET /`
//! lists it all.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use serde_json::{json, Value};
use universe_engine::Context;

use crate::App;

/// The longest a recording runs (s), and the most frames it keeps.
pub const MAX_SECONDS: f64 = 10.0;
pub const MAX_FRAMES: usize = 1500;
/// Sessions kept.
const KEEP: usize = 3;

/// A question on the live port, and where its answer goes: (content type, body).
pub struct Request {
    path: String,
    reply: mpsc::Sender<(&'static str, Vec<u8>)>,
}

/// A recording under way.
pub struct Recording {
    pub dir: PathBuf,
    started: Instant,
    frames: Vec<Value>,
}

impl Recording {
    pub fn seconds(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
}

fn sessions_dir() -> PathBuf {
    crate::save::data_dir().join("freefall").join("sessions")
}

fn stamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Old sessions cleared: all but the newest `KEEP`, and any over a day old.
pub fn clean_up() {
    let Ok(dir) = std::fs::read_dir(sessions_dir()) else { return };
    let mut found: Vec<(u64, PathBuf)> = dir
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse::<u64>().ok()).map(|t| (t, e.path())))
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    let now = stamp();
    for (k, (t, path)) in found.iter().enumerate() {
        if k >= KEEP || now.saturating_sub(*t) > 24 * 3600 {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    // (Live screenshots: the folder emptied.)
    let _ = std::fs::remove_dir_all(sessions_dir().join("live"));
}

/// The live port, listening (None: off, or the port taken).
pub fn listen() -> Option<mpsc::Receiver<Request>> {
    if std::env::var("UNIVERSE_OBSERVE").as_deref() == Ok("0") || std::env::var_os("UNIVERSE_SCREENSHOT").is_some() {
        return None;
    }
    let port: u16 = std::env::var("UNIVERSE_OBSERVE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7878);
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("observer port {port}: {e} (not listening)");
            return None;
        }
    };
    log::info!("observer listening on http://127.0.0.1:{port}/");
    let (tx, rx) = mpsc::channel::<Request>();
    std::thread::Builder::new()
        .name("observer".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let _ = std::thread::Builder::new().name("observer request".into()).spawn(move || {
                    let mut stream = stream;
                    let mut reader = BufReader::new(stream.try_clone().expect("a stream"));
                    let mut first = String::new();
                    if reader.read_line(&mut first).is_err() {
                        return;
                    }
                    // (The rest of the request's head, read and dropped.)
                    let mut line = String::new();
                    while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                        line.clear();
                    }
                    let path = first.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let (reply, answer) = mpsc::channel();
                    let (kind, body) = if tx.send(Request { path, reply }).is_ok() {
                        answer.recv_timeout(std::time::Duration::from_secs(5)).unwrap_or(("text/plain", b"the game didn't answer in time\n".to_vec()))
                    } else {
                        ("text/plain", b"the game has stopped\n".to_vec())
                    };
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&body);
                });
            }
        })
        .ok()?;
    Some(rx)
}

/// Where we are and what we're doing.
fn status(app: &App, ctx: &Context) -> Value {
    let v = &app.view;
    let cam = app.camera.position;
    let star = universe_sim::names::star_name(app.charts.galaxy.stars[v.origin].seed);
    let near = v.system.dominant(cam, &v.positions);
    let b = &v.system.bodies[near];
    let alt = cam.distance(v.positions[near]) - b.surface_radius_at(v.positions[near], cam, app.now());
    json!({
        "time": app.now(),
        "mode": format!("{:?}", app.mode),
        "system": star,
        "near": b.name,
        "camera": { "altitude_m": alt, "position": [cam.x, cam.y, cam.z], "orientation": app.camera.orientation.to_array() },
        "ship": { "name": app.ship.spec().name, "state": format!("{:?}", app.ship.state), "powered": app.ship.powered, "speed_m_s": app.ship.velocity.length() },
        "crew": format!("{:?}", app.v.crew.place),
        "fps": ctx.fps,
        "graphics": app.graphics,
        "debug": app.debug,
        "recording": app.recording.as_ref().map(|r| r.dir.display().to_string()),
    })
}

fn perf(ctx: &Context) -> Value {
    let p = &ctx.perf;
    json!({
        "fps": ctx.fps,
        "frame_ms": p.frame_ms, "update_ms": p.update_ms, "draw_ms": p.draw_ms,
        "render_thread_ms": p.render_ms, "screen_wait_ms": p.wait_ms,
        "worst_ms": p.history.iter().copied().fold(0.0, f32::max),
        "hitches": p.hitches,
        "history_ms": p.history,
        "drawn": { "lines": p.lines, "triangles": p.triangles, "points": p.points },
    })
}

fn resources(app: &App, ctx: &Context) -> Value {
    json!({
        "renderer": ctx.perf.resources,
        "terrain_patches": app.terrain_lod.borrow().patch_count(),
        "crafts": app.v.crafts.len(),
        "crafts_here": app.v.crafts.iter().filter(|c| c.system == app.view.origin).count(),
        "slugs": app.v.slugs.len(),
        "bodies_here": app.view.system.bodies.len(),
    })
}

/// Start recording (debug on, the profiler and a trace with it).
pub fn start(app: &mut App, ctx: &Context) {
    if app.recording.is_some() {
        return;
    }
    let dir = sessions_dir().join(stamp().to_string());
    if std::fs::create_dir_all(dir.join("frames")).is_err() {
        app.say("RECORDING: CAN'T MAKE ITS FOLDER".into());
        return;
    }
    let _ = std::fs::write(dir.join("status_start.json"), serde_json::to_vec_pretty(&status(app, ctx)).unwrap_or_default());
    universe_prof::start_trace();
    app.recording = Some(Recording { dir, started: Instant::now(), frames: Vec::new() });
    app.say("RECORDING: PROFILE, TRACE AND EVERY FRAME".into());
}

/// A frame of the recording: its time and scopes, its picture asked for; stopped when it's long enough.
pub fn frame(app: &mut App, ctx: &mut Context) {
    let Some(r) = app.recording.as_mut() else { return };
    let n = r.frames.len();
    let scopes: Vec<Value> = universe_prof::last_frame().into_iter().filter(|x| x.1 >= 0.05).take(40).map(|(name, ms, calls)| json!([name, (ms * 1000.0).round() / 1000.0, calls])).collect();
    r.frames.push(json!({
        "frame": n,
        "t": r.started.elapsed().as_secs_f64(),
        "ms": ctx.perf.history.back().copied().unwrap_or(0.0),
        "scopes": scopes,
    }));
    ctx.screenshot(r.dir.join("frames").join(format!("{n:05}.png")));
    if r.frames.len() >= MAX_FRAMES || r.seconds() >= MAX_SECONDS {
        stop(app, ctx);
    }
}

/// Stop recording: the session written out.
pub fn stop(app: &mut App, ctx: &Context) {
    let Some(r) = app.recording.take() else { return };
    let trace = universe_prof::stop_trace();
    // (Observing still: the profiler stays on for the port's questions.)
    universe_prof::enable(app.debug == 2 || app.observing);
    let dir = r.dir.clone();
    let _ = std::fs::write(dir.join("trace.json"), trace);
    let _ = std::fs::write(dir.join("status_end.json"), serde_json::to_vec_pretty(&status(app, ctx)).unwrap_or_default());
    let _ = std::fs::write(dir.join("resources.json"), serde_json::to_vec_pretty(&resources(app, ctx)).unwrap_or_default());
    let _ = std::fs::write(dir.join("summary.txt"), summary(&r));
    let _ = std::fs::write(dir.join("profile.json"), serde_json::to_vec(&json!({ "frames": r.frames })).unwrap_or_default());
    app.say(format!("RECORDED {} FRAMES: {}", r.frames.len(), dir.display()).to_uppercase());
}

/// What to read first: the frame times, the slowest frames and where their time went.
fn summary(r: &Recording) -> String {
    let mut ms: Vec<f64> = r.frames.iter().filter_map(|f| f["ms"].as_f64()).collect();
    let n = ms.len().max(1);
    let mut out = format!("session {}\n{} frames over {:.1} s\n", r.dir.display(), r.frames.len(), r.seconds());
    ms.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| ms.get(((n as f64 - 1.0) * q) as usize).copied().unwrap_or(0.0);
    out += &format!("frame ms: mean {:.2}  median {:.2}  95% {:.2}  99% {:.2}  worst {:.2}\n\n", ms.iter().sum::<f64>() / n as f64, at(0.5), at(0.95), at(0.99), at(1.0));
    let mut slow: Vec<&Value> = r.frames.iter().collect();
    slow.sort_by(|a, b| b["ms"].as_f64().unwrap_or(0.0).total_cmp(&a["ms"].as_f64().unwrap_or(0.0)));
    out += "slowest frames (frame, ms; its biggest scopes):\n";
    for f in slow.iter().take(10) {
        out += &format!("  {:>5}  {:>7.2} ms\n", f["frame"], f["ms"].as_f64().unwrap_or(0.0));
        for s in f["scopes"].as_array().into_iter().flatten().take(6) {
            out += &format!("           {:>8.2} ms  {}\n", s[1].as_f64().unwrap_or(0.0), s[0].as_str().unwrap_or(""));
        }
    }
    out += "\nfiles: frames/NNNNN.png (every frame), profile.json (each frame's time and scopes), trace.json (every thread's timeline: Perfetto or chrome://tracing), status_*.json, resources.json\n";
    out
}

fn sessions() -> Value {
    let mut list: Vec<Value> = std::fs::read_dir(sessions_dir())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.parse::<u64>().is_ok()))
        .map(|e| {
            let p = e.path();
            let summary = std::fs::read_to_string(p.join("summary.txt")).unwrap_or_default();
            json!({ "dir": p.display().to_string(), "summary": summary.lines().take(3).collect::<Vec<_>>() })
        })
        .collect();
    list.sort_by(|a, b| b["dir"].as_str().cmp(&a["dir"].as_str()));
    json!(list)
}

const HELP: &str = "FREEFALL observer (127.0.0.1 only)
GET /status          where we are: system, body, camera, ship, crew, fps, graphics
GET /perf            frame times: the last 300, hitches, the parts of a frame
GET /profile         the profiler's report (switched on if it was off)
GET /resources       what the renderer holds; terrain patches; crafts
GET /graphics        the graphics settings; /graphics?shadows=off&textures=on to set
GET /screenshot      capture the next frame; answers with its path
GET /record/start    record (profile, trace, every frame) up to 10 s; /record/stop
GET /sessions        recordings kept, newest first, with their summaries
";

/// The live port's questions, answered (each frame, on the main thread).
pub fn answer(app: &mut App, ctx: &mut Context) {
    let Some(rx) = app.observe_port.as_ref() else { return };
    let pending: Vec<Request> = rx.try_iter().collect();
    for req in pending {
        // (Not observing: nothing's answered but that.)
        if !app.observing {
            let _ = req.reply.send(("text/plain", b"not observing: SHIFT+F3 in the game turns it on\n".to_vec()));
            continue;
        }
        let (path, query) = req.path.split_once('?').unwrap_or((req.path.as_str(), ""));
        let js = |v: Value| ("application/json", serde_json::to_vec_pretty(&v).unwrap_or_default());
        let answer = match path {
            "/status" => js(status(app, ctx)),
            "/perf" => js(perf(ctx)),
            "/resources" => js(resources(app, ctx)),
            "/profile" => {
                let was = universe_prof::enabled();
                universe_prof::enable(true);
                let text = if was { universe_prof::report_text() } else { "the profiler was off: on now; ask again in a few seconds\n".to_string() };
                ("text/plain", text.into_bytes())
            }
            "/graphics" => {
                for pair in query.split('&').filter(|p| !p.is_empty()) {
                    let (name, value) = pair.split_once('=').unwrap_or((pair, "on"));
                    crate::graphics::set(&mut app.graphics, name, matches!(value, "on" | "1" | "true"));
                }
                js(json!(app.graphics))
            }
            "/screenshot" => {
                let p: PathBuf = sessions_dir().join("live").join(format!("shot_{}_{}.png", stamp(), ctx.perf.history.len()));
                ctx.screenshot(p.clone());
                js(json!({ "path": p.display().to_string(), "note": "written a frame or two from now" }))
            }
            "/record/start" => {
                start(app, ctx);
                js(json!({ "recording": app.recording.as_ref().map(|r| r.dir.display().to_string()) }))
            }
            "/record/stop" => {
                let dir = app.recording.as_ref().map(|r| r.dir.display().to_string());
                stop(app, ctx);
                js(json!({ "written": dir }))
            }
            "/sessions" => js(sessions()),
            _ => ("text/plain", HELP.as_bytes().to_vec()),
        };
        let _ = req.reply.send(answer);
    }
}

