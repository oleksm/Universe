# MC-07 equipment: where each model already is

*Ships, 2026-10-10, for the Blender agent's inventory (threads 20261010T150317-blender-deb9 and
20261010T150534-blender-8e05). The MC-07's clamps, gear, ramp and hammers are built already, in one
script, as part of the whole ship. Nothing to remodel: extract them. Extraction is Blender work
(Blender's). This page names the exact sources. Placing parts on hulls (slots, fits) stays with ships.*

## The source

- Script: `~/git/blender/build_mining_ship.py` (builds the whole MC-07 into `~/git/blender/mining_ship.blend`).
- Export in the game: `assets/models/mc07.glb` (`freefall_name: MC-07`, class 3). It has no glTF
  animations: every motion is a driver on a custom property of the ship's root object, listed below.
  An extraction must carry the motion out as data (as `engine-ch-s2/refinement-v11/motion.json` does).
- Also there: `~/git/blender/mc07-assembly/tools/assembly.py`, which already handles `ClampAft` objects.

## Equipment, model groups and motion

| Registry key | Hull slot (`hulls/mc-07.yaml`) | Objects in mc07.glb | Built at (script lines) | Motion (root property: range) |
|---|---|---|---|---|
| `equipment.access.anchor-clamp.mc07` | `clamp_fwd`, `clamp_aft` (access s2) | `ClampFwd_*`, `ClampAft_*`: 9 each (`Stage1-4`, `Pad`, `Jaw0-3`) | `Clamp{tag}_Stage/Pad/Jaw`, ~1090-1122 | `spine_anchors`: 0 stowed, 0.75 extended open, 1 clamped |
| `equipment.gear.front.mc07`, `equipment.gear.rear.mc07` | `gear_0..3` (gear s3) | `Gear_FL_*`, `Gear_FR_*` (front), `Gear_RL_*`, `Gear_RR_*` (rear): 8 each; `gear_0..3` (4) | `gear_leg()` 882; calls 952 (F), 953 (R) | `gear_deploy`: 0 stowed, 1 landed (sequence in the comment at 834) |
| `equipment.access.cargo-ramp.mc07` | `cargo_ramp` (access s3) | `CargoRamp` (1) | `ramp()` 985; placed 995 | `cargo_ramp`: 0 closed flush, 1 lowered (rotation, 996); keep-out `KeepOut_CargoRamp` 1722 |
| `equipment.hardpoint.mining-laser.mc07` (see below) | the two hardpoints (`mount_hardpoint_l`, `_r`) | `HammerL_*`, `HammerR_*`: 15 each (`Hatch0-1`, `Lift`, `RamStage1-2`, `Gimbal`, `Riser1-4`, `Head`, `Boom1-2`, `Breaker`, `Chisel`) | ~1215-1321 | `mining_hammers` 0-1 (stowed to raised and aimed), `hammer_yaw` -180..180 deg, `hammer_pitch` -40..40 deg, `hammer_reach` 0-1, `hammer_strike` on/off |

Rest poses in the export: `gear_deploy` 1, `cargo_ramp` 1 (set at 865, 957); the others 0. The demo
keyframes for all of them are at ~1732-1738.

## Hammer or laser

The model is a jackhammer (a breaker and chisel on a telescoping boom, striking), not a laser. The
registry calls it `equipment.hardpoint.mining-laser.mc07`. Which the MC-07 carries, and what the key and
record say, is the registry's call; asked of the Scientist on 2026-10-10. Until then, extract the hammer
as it is and keep the key.

## The radiators (not built)

No radiator exists in any model. `hulls/mc-07.yaml` has the slots: `radiator_1` (thermal s3) and
`radiator_2..4` (thermal s2), and `loop_1..4`. Each s2 slot offers `mount.thermal-s2`: an envelope
13.8 x 6.89 x 1.65 m, 6.88 t borne, four corner points (each 50.6 kN tension or compression, 25.3 kN
shear). `equipment.thermal.radiator.s2` is 12.5 x 6.26 x 0.08 m. Where on the hull the slots sit is not
set yet: ships places them (to follow), with their clearances from the hammers' reach, the gear, the
ramp's keep-out and the drive plume.
