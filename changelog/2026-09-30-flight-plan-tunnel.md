# 2026-09-30 — Flight plan tunnel: guidance that shows where to face and what to do

## Summary

Playtest feedback on the landing autopilot: "the most confusing thing I've ever seen, the ship goes
sideways then backwards and it does not really follow the guiding targets". The old magenta curve
was a guess, not the path the autopilot flew. User's direction: sideways or backwards is fine, but
the guide must always show where the pilot faces and curve through space to show what to do,
as frames linked by a center line, relative to the ship.

## Changes

- **Flight planner** (`sim::plan`): every frame, fast-forward a copy of the ship under the same
  autopilot, with real gravity and moving targets (station drift, planet rotation), up to 2,000
  steps / 6 hours ahead; ~1 ms. Each point records position (in the target's current frame),
  planned attitude, action (burn / thrusters / coast) and phase. The autopilot flies exactly this.
  Plans match reality: docking 147 s vs 150 s flown; landing from orbit 9,023 s vs 8,950 s.
- **Tunnel in the sky**: a center line along the planned path (orange = main-engine burn, magenta =
  thrusters, dim dashed = coast) with 14 frames, denser near the ship, each shaped and turned like
  the ship at that point (wide like the wings, with a nose tick). You see ahead of time where to
  be and which way to face, including "turn around and brake".
- **Nose cue ⊕**: where the plan wants the nose now; off-screen it becomes a TURN arrow at the edge.
  Replaces the magenta fly-to square. The Hermite guidance curve is gone.
- **Action line**: `TURN: NOSE ON (+), MATCH THE FRAME 176 DEG`, then `BURN: W THROTTLE 100%`,
  thruster keys, or `COAST`; plus `NEXT: BURN IN 0:45` / `ARRIVAL IN 2:30:00`. Shown under
  autopilot too, so you can watch and learn.
- Autopilots now report the attitude they're turning toward (`Command::attitude`).
- The planner turns before deciding thrust, and keeps steps short near the goal (the autopilot's
  reaction time); both were needed for plans to actually arrive.

## Next

- The landing procedure itself is still "hover-cruise" (thrusting against gravity at up to 3 km/s).
  Planned: a physically real deorbit burn, ballistic coast, braking burn and hover. The tunnel will
  show it automatically.

## Follow-up: phase banner + ETA, and steady frames

- **Phase banner** (top center, while cleared): `1 CRUISE AROUND PLANET > 2 APPROACH PAD >
  3 VERTICAL DESCENT` or `1 APPROACH CORRIDOR > 2 ALIGN WITH SLOT > 3 FINAL RUN`, current step
  boxed, plus `ETA h:mm:ss` from the flight plan. Flying by hand, the step comes from the plan.
  The status text and FPS move down to make room.
- **Frames no longer wobble or spin** (playtest: "wobbling and spinning"):
  - Roll is fixed to a reference instead of "shortest turn from wherever we are": ship's top away
    from the planet when landing, along the station axis when docking (`ship::facing`).
  - Frames are pinned to fixed moments of absolute time (tiers: every 5 s near, then 30 s,
    2 min, 10 min, 1 h), so they stay put in space and new ones appear ahead.
  - Planner integration is step-size-aware (thrust to reach the wanted velocity by the end of
    each step, capped by engine capacity), models the real turn rate, and starts with short steps.
    It was chaotic before: plans occasionally ended in a simulated crash (ETA "> 6 H").
  - Only the gravity *difference* between ship and target is thrusted against (a station falls
    around its planet too); cancelling full gravity made off-course docking plans loop.
  - Test: plans made 10 s apart while the autopilot flies agree within 1.6% (landing) and 0.0%
    (docking), roll within 0.2°. 17 sim tests pass.
- The ⊕ nose cue and TURN hint now use the autopilot's aim (end of the turn), not a mid-turn attitude.
- Leftover text "FACE THE SQUARE" is now "NOSE ON (+)".

## Follow-up: exponential frame spacing, clearer action names

- Frames now use doubling tiers (2 s, 4 s, 8 s, 16 s ... of plan time, ~1.5 frames per tier),
  still pinned to absolute time: dense just ahead of the ship, thinning out along the path.
- The `NEXT: … IN m:ss` line was later removed at the user's request (not useful for humans);
  the current coast action reads `COAST - NO THRUST`.

## Follow-up: distances for spatial orientation

- `PATH <distance>` above the ⊕ nose cue and `PATH <distance> TO GO` in the approach panel:
  remaining distance along the planned (curved) path, which differs from straight-line RANGE.
- The next frame ahead is labelled with its distance from the ship.

## Fix: ETA jumping up and down under autopilot

- Playtest: "ETA seems to be jumping up and down even if autopilot is driving".
- Measured: the plan's predicted arrival matches the real autopilot within 2% (docking 146 vs
  150 s, landing from orbit 9,086 vs 8,947 s, gate 259 vs 259 s), so the prediction was fine.
- Cause: some refreshed plans never arrived (the HUD flipped to "ETA > 6 H" for ~1 s, twice per
  docking). The planner carried over the ship's spin rate from when the plan was made and never
  settled it, so "aligned" checks failed forever if the plan was made mid-turn. The planner now
  settles the spin once a simulated turn is complete.
- The HUD countdown is smoothed: it ticks down in real time and eases toward each new
  prediction, snapping only on changes over 10 s.
- Test `eta_counts_down_smoothly`: every refreshed plan arrives and the ETA never jumps by more
  than 0.5 s per frame (docking and gate); landing checked in an ignored long test.

## Frames as real objects: fly through them, not "blasted" by them

- Playtest: frames were "coming too fast and frequent, it feels like frames are coming at the
  ship, not the ship going through frames".
- Causes: every frame had the same *on-screen* size (3% of its distance from the camera), so
  there was no perspective; and the doubling tiers were measured from "now", so new frames popped
  in between existing ones as they got closer.
- Now frames are gates placed along the route:
  - evenly spaced in time, about 12 over the remaining route, snapped to 2 s…4 h steps; the step
    only changes when that drifts outside 6–24 frames, so frames stay put and nothing pops in;
  - a real size in metres (a fifth of the gap to the next frame, 40 m–10 km), so they grow as you
    approach and you pass through them; distant ones keep a minimum on-screen size;
  - naturally denser where the route is slow (final approach) and sparser where it's fast.
- The old "every fourth frame" thinning is gone (not needed with even spacing).
