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
output plus battery rate; engine nozzles bound to real `nozzle_*` empties with shares summing to 1 per slot. The
module draw must stay within plant output plus battery rate.

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
