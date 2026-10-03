# Observing the running game

For looking into what the player sees, without restarting or simulating it (`crates/game/src/observe.rs`).

## Observing (the player)

- **F3** is the player's own overlay (frame times and graph, traffic; twice: the profiler too).
  It opens nothing and writes nothing.
- **SHIFT+F3** is for whoever's helping: **observing** on. The live port answers, the profiler
  and the hitch log run, and a recording starts (up to 10 s or 1500 frames: every frame, the
  profile, a trace). SHIFT+F3 again: all off (a recording under way is written out first).
  The top bar shows OBSERVING, and REC n S while recording.

A session folder, in `~/.local/share/freefall/sessions/<start, s since 1970>/`:

| File | Holds |
|---|---|
| `summary.txt` | read first: frame time mean, median, 95/99%, worst; the slowest frames and their biggest scopes |
| `frames/NNNNN.png` | every frame, full resolution |
| `profile.json` | each frame: its time (ms) and its scopes (name, ms, calls) |
| `trace.json` | every thread's timeline (scopes over 20 µs), Chrome's trace format: open in Perfetto (ui.perfetto.dev) or chrome://tracing |
| `status_start.json`, `status_end.json` | where we were: system, body, camera, ship, crew, graphics |
| `resources.json` | what the renderer held; terrain patches; crafts |

The newest 3 sessions are kept; older ones, and any over a day old, go at start-up. A recording
captures every frame (read back from the GPU), so frames are a little slower while it runs; the
trace shows that cost under `render`.

## The live port (whoever's helping)

`http://127.0.0.1:7878/` while the game runs, opened the first time observing comes on
(SHIFT+F3) and answering only while observing (this machine only; `UNIVERSE_OBSERVE_PORT` to move
it, `UNIVERSE_OBSERVE=0` for none; screenshot runs don't open it). `GET /` lists it:
`/status`, `/perf` (the last 300 frame times, hitches), `/profile`, `/resources`, `/graphics`
(`?shadows=off&textures=on` to set, for this run), `/screenshot` (the next frame, its path
answered), `/record/start`, `/record/stop`, `/sessions`.

While observing, slow frames are written to `hitches.log` beside the
quicksave (it starts over past 1 MB, the last kept as `hitches.log.old`). Not observing,
nothing is logged or written: the frame times are only counted, for F3's graph.
