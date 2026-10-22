# Whole-assembly direction and QC before polish

The explorer's partial ramp sweep did not cover actuator meshes, gear stowage,
stern/cockpit closure or service/support interfaces. Owner review found defects
outside that subset. The valid partial result must not imply assembly acceptance.

Adds `ship-artifact.assembly-state-plan` and
`ship-check.assembly-state-coverage`. G2/G3 direction now records opening/closure
owners, gear states and actual bay reservations, service/support routes and
cockpit/module boundaries. G4/G6 require reconciled component/pair/state/view
coverage across closed, open, intermediate and permitted combined states, with
stern, underside, cockpit and interior witnesses. Required rows need current
results; labels alone cannot pass missing, failed, unknown or stale evidence.

Material manifoldness is separate from enclosure closure: an annular cap can
close shell wall thickness while leaving the cabin open. Requested retractable
gear cannot be replaced by an unapproved fixed-gear scope. Joint contacts,
supports and intended openings require their own evidence beyond intersections.

Independent review verified the source-hashed defect report and inspected three
actual renders: the stern tunnel, canopy/hull seam and underside discontinuity
remain defects/unresolved findings. Both actuator rods have reported hull
intersections in all 33 sampled poses; gear has no authored bays/stowed state.
Blender picking identifies the protruding pieces as a rod and a sill rail, not
two established service pipes. No geometry repair or production approval is
claimed. The real candidate remains in failed assembly review.

All 33 synthetic declaration cases pass after adding per-pair/state/view result
rejection and closure/gear counterexamples. They do not certify real geometry.
Evidence: `docs/diagnostics/ship-assembly-state-audit-20261021.yaml`.
All 47 registry regressions pass; schemas and runtime ship instances are unchanged.
Full registry checks, final page build and six affected page/link checks pass;
the assembly-state check page was visually inspected. There are now 97 workflow
definitions. Existing report gaps are unchanged.
