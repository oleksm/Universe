# Explorer ground stage in the game (engine, 2026-10-24)

Registry record fso 308238a07 (`hull.explorer-study`), ships model 2d2abef64 / fe09a168f
(`assets/models/explorer-study.glb`, sha256 5ba56b39). Main at the commit that adds this file.

What the game builds from the file (`import::commission`, the path `UNIVERSE_HULL` takes):

```
key hull.import.explorer_study visual Some("assets/models/explorer-study.glb") sha 5ba56b39d25098985064ff5839f5fe37cfd8c7178b4fcff68453cb6fb9184d24 commissioned false dry_mass 6829.395 kg fit 0 slots 0 thrusters 0 mechanisms Some((3, 2, 6)) limits0 Some("not commissioned: no Power: every ship must carry one")
```

Live: `UNIVERSE_HULL=assets/models/explorer-study.glb UNIVERSE_SCENARIO=settlement` (a pad at Port Trethi, Rime F,
airless). The player's ship is the Explorer, landed (alt 2 m, 0 m/s), drawn from its own model: the rear two-leaf
ramp and the belly ladder deployed to the ground, gear down. Captures: `out/review/explorer-ground/pad.png`,
`pad_yaw-0.6_crop.png` (from aft), `pad_yaw0.9_crop.png`. Static boarding checks:
`docs/ships/explorer-final-stage-check.txt`.

Fixed on the way:
- A stage was refused joining the game's hulls (`turn_rate must be positive (0)`): a stage is now checked as a body
  (mass, size, strength), not as a flyer. Without that the game silently kept the previous ship.
- Its model was refused by the renderer's preparation bound (130 MiB estimated against 128): now 256 MiB, and the
  procedural shape was drawn in its place.

Open: the content key is `hull.import.explorer_study` (from the path), not the record's `hull.explorer-study`. No
flight: it makes no thrust (no power, drive or thrusters fitted).
