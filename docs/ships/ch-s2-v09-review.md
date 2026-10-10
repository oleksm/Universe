# ch.s2 v09 rest export and motion account: registry review (2026-10-10)

Of blender `engine-ch-s2/refinement-v09/about.yaml` (`evidence.motion`) and `review/runtime-handoff-checks.json`.
Checked by sweeping the gimbal over its envelope (pitch and yaw ±1, and the four diagonals, with the stated
normalisation) using the account's own pivot, axes and anchors.

## Fits

- Size: 2.305 x 1.31 x 1.31 m against the record's 2.3 x 1.5 x 1.5 (sorted sides within the rule; the rim matches
  CHE2-01's 1.31 m). Origin at the mount face, nozzle along -Y: as the asset contract and ship import want.
- Static only, markers present, part selectors unique, binds within 2e-7 m: fine for an install when accepted.
- Gimbal: pivot 0.43 m aft of the mount face (exported space), pitch about +X, yaw about -Z, nested, normalised
  `d = max(1, |input|)`, limit 0.07 rad = the record's `function.gimbal`.

## Findings

1. **Actuator overlap has no margin.** At full stroke both actuators reach exactly their 0.060 m minimum overlap
   (pitch: length 0.414-0.441 m against 0.282 body + 0.219 rod; yaw: 0.461-0.479 m against 0.310 + 0.229). Any
   tolerance, any wear, any thermal growth and the rod leaves its guide. Give each 10-15 mm more engagement (a longer body
   or rod), or say why zero margin is acceptable. Owner: blender.
2. **Hoses need more than ends, diameter and length** (engine's contract, d5031648): the rest centreline (given), end
   tangent axes, routing guides with parent frames, a minimum bend radius and clearances, so the game's tube solver has
   one answer. Owner: blender, values below for the bend radius.
3. **Bend radius, derived (review):** the gimbal-side end moves up to 41 mm (hydrogen) and 27 mm (oxygen) over the
   envelope against a 0.263 m free length; spread as an S-offset that asks for a bend radius of about 0.42 m and 0.63 m.
   The rest shape is a loop (0.263 m of hose for a 0.109 m span), so the true minimum is set by the loop, not the
   offset: it is the hose's rest bend that must stay above the product's minimum. A corrugated stainless hose with braid
   for this duty is the usual choice; its dynamic minimum bend radius is the maker's figure, not ours: state it as an
   assumption (and its source) in the account. Owner: blender; registry adds a flex-line product when one is needed.
4. **The turbulence is the owner's call** and stays out of the model; engine's sequence has no gimbal command yet
   (its step 4a), so even steady steering waits for that.

## What comes next

`freefall-motion/1` (registry, now: engine asked for the schema before any consumer): the account's `evidence.motion`
becomes a checked `motion.json` in the package; the installer checks nodes against the model, parents, rigid binds,
the limit against the record, actuator overlap over the envelope and hose length against end distance.
