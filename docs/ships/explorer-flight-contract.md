# Explorer: minimum flight vs uncommissioned stage, as main is now (engine, 2026-10-23)

Ships `20261023T220633-ships-daf7`: the owner says finish the Explorer; command stays deferred. This is what main
does today. Nothing here is a mission or design acceptance.

## Uncommissioned stage (no flight)

A registered hull (its record names the model by exact path and `model_sha256`) with a **whole mass** (`goods::
hull_mass` Recorded or Parts; partial or none is refused). Its fit as recorded, empty allowed. It loads, is drawn,
walked, ramp/hatch/ladder in their discrete states. `ClassSpec::commissioned` is false, and the reason is
`limits[0]`. It can't be flown.

## Minimum flight (commissioned)

Every base block filled with a working (not inert) module: power (a plant, or batteries alone if their rate
covers the draw), drive (or engine slots with a main push along the nose), thrusters (or cold-gas engines), tank,
flight computer, transponder, sensors (camera or altimeter suffice), comm, **life support**. Its draw within plant
output plus battery rate; engine nozzles bound to real `nozzle_*` empties with shares summing to 1 per slot.

- **No command station or cockpit is required to fly.** The ship is flown by the pilot's controls through the
  engine's pilot path (test drive). Without a `cockpit` node there's no seat to stand up from (getting up says "no
  room to stand"; out by an exit), and the seated eye is the ship's default.
- **Uncrewed is not a mode today.** Life support is a base block, so a fit without it is a stage, not a flyer. An
  uncrewed prototype needs a record-level declaration (e.g. crew 0) that waives life support. That's a registry
  schema field plus a small engine rule; ask and I'll add the engine side.
- **No "externally commanded" concept** beyond that: the test drive's pilot is the commander.

## What's simulated, what isn't (stated in `limits`)

Simulated: rigid-body flight with each nozzle at its place and push; fuel and own cold-gas pools, finite; batteries
drained, recharged from spare, the ship dark when plant plus packs can't carry the draw; ground contact on the
`gear_*` feet with the fit's leg limits; mechanisms in discrete states with ground-contact checks.

Not simulated:
- heat (engines' heat aboard, radiators: carried mass only);
- gimbal (fixed), throttle floor or on/off (thrust 0 to full);
- ignition, restarts, chill-down; thrust at rated whatever the ambient pressure (fine in vacuum);
- cold gas at rated exhaust, not from tank pressure or temperature;
- camera picture and altimeter reading;
- mechanism motion, and gear state gating leg contact;
- hull-owned edited gear with no leg record has no landing limits of its own.

## Test route (bounded, test only)

Registry record (whole mass, fit, thrusters) → `hull_import_review` clean → the ships' test drive on an airless,
low-g body (no drag, no aero, no ambient pressure effects: the simulated set is honest there) → a short flight and
landing recorded with the flight recorder. No traffic sim, no mission claim.

## For the first-flight envelope (ships `20261023T231645-ships-d95e`; main cc0e8f20c)

The owner chose one person, no cargo, short trips on an airless moon. What the game does, exactly:

- **Life support:** a base block and its power draw only. No air, water, CO2 or endurance is simulated; air and
  water tanks are carried mass. The trip's duration is not limited by it in game.
- **Heat:** not simulated for the ship. One exception: a battery pack heats from its own losses (1 − efficiency)
  and cuts off at its record's operating limit. Nothing cools it yet (it never sheds heat; radiators are mass only),
  so a long run at high draw ends at the cut-off, not at the pack running empty.
- **Throttle floor:** not enforced. Every nozzle's thrust runs 0 to full; the record's floor or on/off is in
  `limits`. Short 20% pulses are honest only as far as the engine could really hold 20%.
- **Plant part-load:** fuel burned is exactly the energy taken over (fuel energy × the plant's efficiency): constant
  efficiency at any load, no idle burn, no minimum load, no start-up. Its spare recharges the batteries (rate-capped)
  and burns fuel for it.
- **Batteries:** drained for what the plant can't give, recharged from its spare, dark when plant plus packs can't
  carry the steady draw (e963b9bf0).
- **Recorder:** the flight recorder keeps each ship's last 15 s at 0.25 s steps, and files an incident (with the
  trace) only when a ship is wrecked. A whole session's inputs are recorded with `UNIVERSE_RECORD=<file>` and
  replayed with `core/sim/examples/replay.rs` (seed-only). There's no landing report: a safe touchdown files nothing.
  Ask if a test needs a trace dump on landing; it's a small engine addition.

## Airless moons in the game (registry `20261023T231721-registry-78d0`; seed 1984, home system Treistun)

Surface gravity at runtime is `mu / r²` (`Body::surface_gravity`: G × mass / radius², radius the body's rail
radius). The seed's figures, unless the celestial registry's overrides change a body's mass or radius. Air: only
temperate (Terran) worlds get an atmosphere; every moon is airless. All the moons below have terrain (cratered),
so a ship can land on them. Ports are where play can start or dock; a moon without one is still landable.

| Moon | Parent | g (m/s²) | Radius (km) | Ports |
| --- | --- | --- | --- | --- |
| Cinder B I | Cinder B | 1.415 | 1687 | Port Seiwiti |
| Anvil C I | Anvil C | 1.114 | 1328 | Port Fabindum |
| Heath D I | Heath D (home) | 1.281 | 1527 | Port Sirnendis |
| Drum G I | Drum G | 1.872 | 2232 | none |
| Drum G II | Drum G | 1.006 | 1199 | none |
| Drum G III | Drum G | 0.701 | 836 | none |
| Pale H I | Pale H | 0.702 | 837 | none |
| Pale H II | Pale H | 2.178 | 2597 | Port Weisonum |
| Hush I I | Hush I | 0.971 | 1157 | none |
| Hush I II | Hush I | 1.096 | 1307 | Port Aipika |
| Hush I III | Hush I | 0.471 | 561 | none |
| Verge J I | Verge J | 1.254 | 1496 | none |
| Verge J II–IV | Verge J | 0.47–0.63 | 560–751 | none |

In 1–3 m/s² with a port: Cinder B I, Anvil C I, Heath D I (the home world's moon, nearest), Pale H II, Hush I II.
Rime F (4.06 m/s², airless, Port Trethi) is the nearest airless world above that.

Runtime limits asked: no life-support depletion (air, water, CO2) is simulated. A fuel cell's part-load is
constant efficiency with no heat simulated (fuel burned = energy taken / (fuel energy × efficiency)); its heat isn't
tracked anywhere.
