# MC-07 ramp hinge and rigid fixture consumer

Use the authored CargoRamp mesh origin/axis for imported ramp rendering and
collision, rather than the separate boarding hatch. Current hull regression and
full workspace tests pass. Added independent 65-pose source/GLB evaluator for
five rigid parts and optional ten-node references. Static asset replacement
remains blocked on final hull fit and live motion/boarding validation; see
`docs/ships/mc07-cargo-ramp-runtime.md`. No assets installed or branches merged.
