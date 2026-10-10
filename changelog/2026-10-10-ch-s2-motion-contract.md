# CH-S2 runtime motion handoff

- Documented the engine's existing rigid-part transform support, static registry
  loader and unsupported clips/skins/morphs/hoses in `docs/ships/ch-s2-motion-contract.md`.
- Specified the exported coordinate/bind-transform, actuator attachment and
  normalized two-axis steering data needed from Blender. No runtime behavior or
  installed asset changed.
- Confirmed the clipless integration sequence for registry: schema and exported
  frames, mounted neutral equipment, applied gimbal state, rigid linkage, then
  procedural hoses with routing constraints and dynamic mesh updates. Updated
  the v09 pivot and separated technical readiness from asset acceptance.
- Superseded the CH-S2 hose route with registry's tied-bellows duct proposal.
  Required fixed-length closure, explicit joint freedoms/ratings and roll, and
  separated rigid hardware motion from the unresolved bellows surface rendering.
- Reviewed proposal-v03 with an independent 513-pose kinematic sweep. Documented
  links export/orientation-validator gaps and a conditional rigid-ring bellows
  prototype, with the Hermite surface retained as review reference.
- Added a repeatable Rust v04 fixture consumer check using the actual PBR loader.
  All 66 node poses and baked-part vertex transforms match Blender references;
  documented neutral-calibrated Hermite sleeve frames and pending runtime gates.
- Recorded registry agreement/publication of the v04 bellows evaluator and
  closed that handoff dependency, with fixture/schema mapping differences noted.
