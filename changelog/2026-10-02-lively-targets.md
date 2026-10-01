# Guidance for lively targets

A target that manoeuvres hard (a ship burning or dodging; battles will be full of them) makes a
path that doesn't hold: rebuilt every frame, its arches jump. So the follow guide watches how
lively its target is: its acceleration beyond gravity, smoothed over half a second (stations,
gates and rocks only fall, so they never count).

- **Over 1 m/s²** (back under 0.5 m/s², so it doesn't flicker): no arches, the path line alone.
  Each rebuilt line is kept for 1.5 s and fades out behind the new one, riding with the target
  so it doesn't streak off at orbital speed.
- The HUD says so: "MINER 1 MANOEUVRING (2.0 M/S2) - PATH ONLY".
- Dev scenario `orbitshiplively` (the target burning at 3 m/s²); `orbitship` (cruising) keeps its
  arches.
