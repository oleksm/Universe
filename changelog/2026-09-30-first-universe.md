# 2026-09-30 — Engine and first playable universe

## Summary

Built from scratch in Rust: a retro vector engine, and a seeded,
simulated universe you can observe or fly through. Everything is drawn as wireframes with hidden
lines removed, at 270 lines of resolution upscaled with sharp pixels.

## Engine (`crates/engine`)

- Window, main loop and input (winit 0.30), rendering with wgpu 30.
- Low-res offscreen framebuffer (270 px tall, width follows the window) upscaled with nearest filtering.
- Immediate-mode draw list (`Frame`): lines, occluding solids, world points, sky points, HUD lines/rects/text.
- Hidden-line removal: models draw black occluding faces, with edges biased toward the camera.
- `WireModel`: lat/long globes, convex-hull models built from a point list, detail loops.
- 64-bit world positions, made camera-relative before reaching the GPU; reversed-Z infinite projection.
- 8x8 bitmap font for the HUD.
- Procedural synth audio (cpal): engine rumble, drone, square-wave tones, noise bursts.
- PNG screenshots (F12, or `UNIVERSE_SCREENSHOT=path` to capture a frame at startup and exit).

## Simulation (`crates/sim`, no graphics)

- Galaxy: 40,000 stars in a two-armed spiral with a bulge; spectral classes O to M.
- Star systems generated from each star's seed: rocky planets inside the frost line, gas/ice giants
  beyond it, rings, moons within each planet's Hill sphere, and a station around the most
  habitable rocky world. Names from letter pairs.
- Kepler orbits (analytic position and velocity), so time warp up to 10,000,000x stays stable.
- Ship physics: leapfrog integration under gravity from all bodies, adaptive substeps, warp limiter.
- Landing and docking (slow touch-down), crashing and respawn at the home station.
- Hyperdrive: speed proportional to clearance from the nearest obstacle; drops out automatically
  when the target is dead ahead. Crossing between systems re-centres coordinates on the new star.
- Save/restore of the full state; JSON round-trips exactly.
- Tests: home system generation, orbit stability under warp, orbital velocity, hyperjump arrival,
  save round-trip.

## Game (`crates/game`)

- Observer mode: orbit any body, cycle bodies and nearby stars, zoom from a hull plate to the whole galaxy.
- Pilot mode: the starting ship, cockpit and chase views, mouse or keyboard flight.
- HUD: status, focus/ship readouts, prograde/retrograde markers, target bracket, 3D scanner,
  messages, F1 help panel, label collision avoidance.
- Orbit paths, star halos, planetary rings, galaxy starfield with additive glow.
- Quicksave/load (F5/F9) to `~/.local/share/universe/quicksave.json`.
- Dev scenarios for quick visual checks: `UNIVERSE_SCENARIO=<name>` with system, inner, planet, giant,
  rings, galaxy, neighbours, cockpit, hyper, landed.

## Fixed along the way

- Galaxy view rendered black: squaring galactic distances overflowed f32 in `Camera::look_at`.
- Hyperdrive flew into stars, then dropped out immediately near planets; now it drops out only for
  obstacles on the flight path.
- Save/load shifted the clock slightly; enabled exact float round-tripping in serde_json.

## Known gaps

- Sound opens the audio device without errors but hasn't been listened to yet.
- Flight feel (mouse sensitivity, turn rates) hasn't been tuned by hand.
- Stations can be docked with, but there's nothing to do there yet.
