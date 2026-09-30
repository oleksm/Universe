# 2026-09-30 — Aggression; softer sun rays

User: "The pilot who opens fire on an unauthorized target (pretty much anyone) gets a 10 min
aggression timer. You will see aggressed pilots red. Shooting aggressed targets won't trigger
aggression on the shooter. Small item: sun rays are too noisy, make them more transparent."

- **The rule** (world, combat phase): when a ship's gun or laser hits a ship that isn't aggressed,
  the shooter becomes aggressed for `AGGRESSION` = 10 game minutes (`Ship::aggressed_until`,
  `Ship::aggressed(now)`). A further hit on an innocent ship restarts the timer. Hitting an
  aggressed ship (fair game) is no crime.
  - "Opening fire" counts at the first hit, which is when the target is known. Collisions don't
    count.
  - `ShipEvent::Aggressed { until }` tells the shooter: "AGGRESSION - YOU FIRED ON AN INNOCENT SHIP /
    YOU ARE FAIR GAME FOR 10 MINUTES".
- **Seen in red**: an aggressed ship's radar contact marker, its bracket and arrow when locked, its
  scanner blip, its name label and model in the scene, and `LOCK <NAME> AGGRESSED` in the readout
  (`Contact::aggressed`). Your own status shows `AGGRESSED m:ss - FAIR GAME TO ANYONE`, in red.
- Pirates turn red once they strike, so traders' defenders (and you) may fire on them freely.
- A replacement ship (after being wrecked) starts clean.
- Test: `opening_fire_aggresses_the_shooter_unless_the_target_is_fair_game`. The dev scenario
  `pirates` now happens 30 km out (clear of the station's shelter), names its pirate and trader,
  and stops at the first hit.
- **Sun glare**: the halo alpha went from 0.55 to 0.3, and the rays from 0.85 to 0.35. The core
  stays bright.
