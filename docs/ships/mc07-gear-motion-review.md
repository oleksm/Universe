# MC-07 gear analytic deployment review

The independent diagnostic evaluator reproduces all12 nodes at129 reference
poses for each of the front/rear, left/right installed variants. Maximum matrix
component error is5.66245e-7 front and6.48201e-7 rear, below the existing2e-6
gate. Rotation components are dimensionless; this is not a mesh vertex-distance
or clearance bound. Reports pin the exact source and reference fixture hashes
under `docs/diagnostics/mc07-gear-motion-v1/`.

`tools/review/gear_motion_check.py` reads authored unconstrained parent TRS,
evaluates a restricted arithmetic/min/max driver grammar, composes parents,
then independently solves actuator tracking. It never interpolates or reads
reference pose matrices to drive motion. Right variants apply the authored
proper S*M*S conjugation and node rename. No timing, landing suspension,
contact forces or animation clips are introduced.

Use the exact driver expressions: doors finish atg=.25, not the .2 in old
fixture prose. Strut/pad motion starts at.2 and overlaps door motion. Preserve
1.5708 and1.8182 rather than replacing them with ideal pi/2 or1/.55. Front/rear
piston travels2.8/2.3m are deployment geometry, not the registry landing stroke.

Blender's read-only129-pose probe confirms the antiparallel DAMPED_TRACK
convention: pi around preconstraint local+X. Ordinary tracking is shortest arc
from current unconstrained Blender+Y (glTF-Z) to the opposite anchor. The
already-constrained rest bind is not an unconstrained frame. The tool defaults
to refusing antiparallel targets; the explicit `--half-turn preconstraint-x`
option selects this measured convention. Unsupported constraints, driver
syntax/dependencies, coincident anchors and tracked-parent descendants refuse.

Use `--precision f32` to reproduce source arithmetic. The float64 hierarchy
perturbed near-antiparallel front ActBody tracking enough to produce1.167e-5
component error atg=.203125; this failed the unchanged2e-6 gate. Float32 parent
composition before the tracking calculation restores agreement. This numerical
sensitivity must be retained in the eventual Rust consumer and regression
fixtures; it is not evidence for loosening clearance or matrix thresholds.

Authoritative contracts: Blender
`mc07-equipment/gear-runtime-handoff/gear-motion-contract.yaml` and its hashed
tracking-probe.json; ships `mc07-combined-fit/gear-runtime-handoff.json` pins
all four installed GLBs, proper mounts and replacement node lists against
combined hull e001013154e1410127145ffeb6d774372430bddb319541438272edc85e4de473.
The v02 fixed supports remain required. Static installs are unchanged.

Next runtime gate: port this agreed evaluator into the articulated consumer,
verify GPU group transforms/contact markers on the combined hull, then integrate
landing/contact behavior. Native suppression remains disabled. No collision,
continuous/off-grid motion or structural qualification is claimed here. Hammer
chisel/well tangency and ramp/radiator holds are separate and unchanged.
