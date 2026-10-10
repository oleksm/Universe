# Respect deferred MC-07 lift sizing in the balance test

Exclude only hull.import.mc07 from the empty-hold hover target, as the owner's
2026-10-05 registry revision defers hold/tank/lift sizing until flight trials.
Retain allocator checks and all other hull hover assertions. No ratings or
simulation behavior change. Record ships' exact anti-slip rib classification
for the separate ramp grounding review; no export defect or repair.

Validation: full workspace tests pass without skips; registry build passes.
