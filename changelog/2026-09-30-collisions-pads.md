# 2026-09-30 — Ship collisions, landing pads, traffic control queues

User: "ships are just going through each other. Ships need to collide: if the speed is enough both
crash, if not a proper change of impulse. Spaceports need landing pads, and ships are cleared for a
specific pad. If all pads are busy, the dispatcher queues them: ships fly around without blocking
pathways and try to avoid colliding. A 3×3 grid of pads per spaceport for now." Later: "collisions
are inevitable, that's fine. They should just try to avoid them, but it may not always be possible
if the scale of settlement doesn't fit."

## Collisions

- **Kernel `pairs`**: moving spheres swept against each other over the step (relative motion, so
  they can't step through each other at km/s), with a spatial grid (2 km cells) to keep it cheap.
  `bounce_pair` gives the momentum-conserving bounce with restitution `e`, and the kinetic energy
  lost.
- **World `collisions`**: after every ship's turn and the combat phase:
  - ships bounce with `RESTITUTION` = 0.3, and are pushed apart to touching;
  - the energy lost goes into both hulls, half each (`damage::hit`, cause "COLLISION");
  - a ship on the ground stands firm (infinite mass) but takes its half;
  - docked ships (inside the station) and ships in hyperdrive or transit don't take part;
  - slower than 0.5 m/s, touching ships just ease apart: no event, no damage.

  With 90 t ships, two ships wreck each other at about 44 m/s closing speed, and a ship hitting a
  grounded one at about 31 m/s. A 4 m/s nudge dents 1.6% of the hull.
- Events: `ShipEvent::Collided { with, speed }`. Stats: `collisions`, and `collision_losses`
  counted apart from weapons kills. Kill feed: "X WRECKED IN A COLLISION WITH Y".

## Pads and holding

- **Pads**: 9 per spaceport (3 × 3, 150 m apart; `spaceport::pad_direction`, `pad_at`).
- **The pad book** (world `pads::PadBook`, traffic control): a landing clearance comes with a pad
  of your own (`Clearance::pad`: `PadSlot::Pad(k)`), the free one nearest the middle. When all
  are taken, `PadSlot::Hold(n)`: you're queued, and hold in the ring over the port (6 km up, 4 km
  out, a place per queue position) in `Phase::Hold`. You ask again every frame and descend when
  your turn comes (`TrafficEvent::PadAssigned` / `Holding`).
  - A pad stays booked while its ship is on its way, standing on it, or climbing out of its column
    (below 5 km, within 800 m).
  - A ship standing on a pad occupies it even if it never asked (spawned settlers).
- **The corridor book** (`pads::CorridorBook`): a station's docking corridor or a gate's run takes
  one ship at a time. It's asked for within 12 km, and released once the holder is on its final
  run within 1.5 km, so ships follow each other through, spaced out. Launching from a station
  waits for it.
  - Ships waiting for a corridor hold at their own place on two rings of 12 beyond its entry
    (`computer::hold_for_corridor`), on thrusters, facing it.
- **Shelter** is now 9 km (was 6 km), so pirates don't prey on ships waiting their turn under
  traffic control's eye.
- Pirates keep 400 m from other ships.
- Settlers spawn spread over the port's pads.
- HUD and scene: `LAND <PORT> PAD 5` or `HOLDING (2 AHEAD)`. At the port, 9 pad squares: yours
  bright with a beacon, taken ones amber.

## How the traffic fared (100 settlers, 2 game hours at 1×)

| | Collision wrecks | Stops | Gate transits | Notes |
|---|---|---|---|---|
| Collisions only | many (432 in 10 h at 20×) | | | stacking on one pad, corridor conflicts |
| + pads, occupancy | 13 in 30 min | | | gate approach convergence left |
| + corridors, waiting rings | 4 in 30 min | 446 | 313 | pirates ramming |
| + corridor release at 1.5 km, 24 places, pirate separation | 2 | 442 | 300 | pirates farm the waiting rings: 425 kills |
| + shelter 9 km | 20 | 437 | 320 | kills 71; more ships survive to crowd ports |

Before collisions and queues it was about 850 stops and 930 transits in 2 h. Queueing halves
throughput, and 1,000 settlers gridlock the home system: capacity, not avoidance, is now the limit,
as the user expected. More pads and corridors, faster final runs and smarter routing are the
levers.
