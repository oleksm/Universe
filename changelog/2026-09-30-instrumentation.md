# 2026-09-30 — Instrumentation: interaction tests, flight recorder, one traffic control

User: "let's have all of that, let's do instrumentation. I want you to stop running those slow
simulations once and forever." (See `2026-09-30-retrospective.md`; this is its remedy list, all
four.)

1. **Interaction tests** (`crates/sim/tests/interactions.rs`, 5 tests, about 0.5 s together):
   - two ships cleared for one gate take turns: one waits on the ring, both go through, no
     wrecks;
   - 9 ships on the 9 pads, a 10th holds (`Hold(0)`, high over the port), then lands on the pad
     that's freed (pad 5);
   - a launch and a docking share a station's corridor without meeting;
   - pirates leave a ship 4 km from the station alone, and hunt one 30 km out;
   - the recorder files a head-on collision with both traces.

   Supporting API: `Universe::{craft_set_nav_target, craft_request_clearance,
   craft_toggle_autopilot}` drive one craft's avionics through its own bus, as its pilot would.
2. **Flight recorder** (`sim::recorder`): every ship sampled every 0.25 s of game time, the last
   15 s kept. Every wreck is filed as an `Incident`:
   - crashes, from the player's and crafts' events;
   - collisions and weapons kills, from the combat phase.

   An incident holds the ship's trace and, for collisions and kills, the other ship's. Its
   `Display` prints each sample: state, speed, throttle, thrusters, hull, armed or hyperdrive,
   clearance (target, phase, pad), route, departing, waiting for a corridor, hunting, and against
   the other ship the separation and closing speed. `Universe::recorder`.
3. **One traffic control with explicit claims and releases** (world `pads::TrafficControl`, which
   replaces the pad and corridor books and the sim's per-frame inference):
   - **claims**: `request_pad`, `request_corridor`;
   - **releases**: `release(ship)` from events (clearance cancelled, docked, through a gate,
     wrecked, respawned, left the system), plus `release_corridor`;
   - **physical facts each frame** (`presence`): who stands on or over which pad, and who's done
     with the corridor it holds.

     A pad is held until its ship has been there and gone. Standing on a free pad occupies it.

   Two places dropped a clearance silently: the route autopilot moving to its next hop, and a
   pirate starting a hunt. They'd have leaked holds, so both now emit `ClearanceCancelled`. Unit
   tests: queue order and release, arrival and departure, squatters, corridor turn-taking.
4. **Less friction when adding things**:
   - `computer::AutopilotInput`: one struct instead of 9 parameters;
   - the game's message and sound matches have quiet defaults, so a new event needs no edits
     there unless it should say or sound something. The avionics' `observe` stays exhaustive, to
     guard correctness.

- `docs/architecture.md` has a new section, "How changes are verified".
- Standing rule, saved in memory: **no long whole-traffic simulations in the development loop**.
  Use interaction tests and the recorder.
