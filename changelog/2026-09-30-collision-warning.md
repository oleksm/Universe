# 2026-09-30 — Collision warning (toggle I)

User: "where is my collision marker? I used to have that red cross and path to collision. We need
to make it togglable. When it is on it shows me trajectory, impact cross, and time to impact. The
collision mode should detect not just planets, but all collidable objects including stations,
gates, other ships. Limit the distance to 100 km."

The old red cross was part of the landing guidance: a free-fall prediction against the pad's
planet, shown only with a landing clearance. It stays there. This adds a general ship system.

- **avionics `collision`**: `predict` flies a copy of the ship ahead as it is set now (engine and
  thrusters holding) through the kernel's `simulate`, with a driver that stops on any contact
  and flies on through gate openings (triggers).
  - It hits what the kernel's colliders hit: body surfaces with terrain (the sea is named),
    station hulls and slots, and gate rings.
  - Other ships: radar contacts within range, propagated under the same gravity. (A first
    version moved them in straight lines, and in orbit they parted from the path within
    seconds, a missed warning.) Closest approach per stretch; a hit is within 2 × the ship's
    radius.
  - Reference: the nearest station or gate within range, else the dominant body. The path is
    stored as offsets from it and drawn moving with it.
  - Limits: `RANGE` = 100 km from here in that frame, `HORIZON` = 30 min, and a substep budget
    (8,000).
  - Timing: substeps are kept short near the ground in proportion to altitude over speed, so a
    ground impact is timed to a fraction of a second (the kernel's orbital-scale substeps are
    about 8 s).
- **kernel `Span::contact_step`**: the fine step within 30 km of small colliders is now a
  parameter. Flight passes `FINE_STEP` (0.05 s, unchanged). The prediction passes about 20 m of
  relative motion, which is enough for km-sized hulls and swept ring crossings, and is what makes
  a 100 km look-ahead near a station affordable.
- **Toggle I** (`Avionics::collision_warning`, saved). It runs 5 times a second, about 0.5 ms each
  near the station, shown in the perf corner.
- **HUD**:
  - The path fades from cyan to red toward the impact, with a red 3D cross at the impact point.
  - The impact is bracketed on screen as `IMPACT m:ss`, with the range.
  - Status line: `COLLISION <WHAT> IN m:ss AT <speed>` (red), `PATH CLEAR 100 KM`, or how far it
    looked when the budget ran out.
  - Action grid: a new `I COLLIDE` lamp, lit when on and red on a collision course. The grid is
    now 5 wide.
- Tests: `warns_of_the_station_ahead_and_the_ship_in_the_way` and
  `clear_when_moving_away_and_ground_when_falling`. Dev scenario: `collision`.
