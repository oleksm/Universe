# Request to the integration session: build hulls to match the registry

*From the ships session on `ships` to the integration session on `main`, 2026-10-05. The user's
direction: "We need to align with the registry where possible. If there is conflict ask integrator
to build to match registry." File and line references are to `ships` at the commit that adds this
file.*

## Why it came up

The interior studio's MODULES tool lays out what a ship carries. It now reads the fit from the
hull's record in the registry (`standards/SFO/metadata/hulls/mc-07.yaml`). The game builds the
MC-07 with a different fit and a different hold, so the studio and the flying ship disagree.

## The conflicts (MC-07)

| What | The registry (`mc-07.yaml`) | The game builds | Where the game decides it |
|---|---|---|---|
| Fit | Its `fit`: 12 items, the cargo slot left empty ("its hold is its ore bay") | The cheapest module for every slot, so the cargo slot gets `Cargo Racks 4 T` (0.35 t, 6 m³) | `crates/world/src/import.rs:90` calls `stock_fit`, `crates/world/src/design.rs:479` |
| Hold, mass | `capacity.hold`: 1,200,000 kg | 4,000 kg, the racks' capacity | `crates/world/src/ship.rs:501` |
| Hold, volume | `capacity.hold_volume`: 669 m³ (the ore bay; its depth is a guess marked to review) | 6 m³, the racks' volume | `crates/world/src/ship.rs:504` |
| Dry mass | Parts plus the 12 fitted items (11.44 t of equipment) | 153.6 t, which includes the racks' 0.35 t | follows from the fit |

Fuel agrees: both say 4,000 kg (`capacity.fuel`, the `equipment.tank.s0` it fits).

## What is asked

1. **Build a hull the registry describes with that record's `fit`,** not `stock_fit`. Keep `stock_fit`
   only for hulls the registry doesn't describe.
2. **Take a registry hull's hold from its `capacity`** (`hold` and `hold_volume`), not from racks.
   The MC-07 carries ore in its bay with no racks fitted.
3. **Let the dry mass follow** (it drops by the racks' 0.35 t).

The studio already reads the registry this way: `fit_of` in `crates/game/src/interior.rs` finds
the hull's record by its model, as `import.rs:87` does for its mass.

## For the registry session, not a conflict

The equipment records have `physical.length`, `width` and `height` in their schema but none are
filled. The studio uses them as soon as they are; until then it sizes each module from its volume
in the game's proportions for its kind (a tank as a ball).
