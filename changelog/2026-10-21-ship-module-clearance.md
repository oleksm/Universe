# Inherited modules and their full clearance requirements

The reuse correction must cover inherited geometry as well as new work. The
workflow now requires a semantic owner for source objects, linked collections
and build outputs, including old pilot seat/console geometry and hidden helpers.
Retire active substitutes, stop their regeneration and invalidate affected
previews, exports, layout, BOM/mass and clearance evidence. Historical candidates
remain explicitly withdrawn rather than being copied forward.

Adds `ship-check.module-clearance` at G3 and G6. QC uses actual product models,
record/consumer frames and unscaled placements. It checks occupied space,
mount/holder/fastener/tool room, installation/removal, user approach/egress,
operational motion and service access. Requirements and allowances retain
sourced, derived or provisional authority. Missing data remains unresolved;
boxes and declarations cannot qualify geometry or runtime behavior.

Independent review found and repaired two false passes in the project screening:
an unresolved service clearance and an unowned hidden source object. All 22
synthetic declaration/AABB cases now pass. The 113-object Blender triage roster
and corrected source hash were verified; remaining semantic ownership and actual
product fit/clearances are not accepted. No module, runtime schema or ship
instance is added by this change.

Evidence: `docs/diagnostics/ship-module-clearance-audit-20261021.yaml`.
All 47 registry regressions pass; the workflow contains 95 definitions.
Full registry checks and four affected page/link checks pass. The clearance-check
page was visually inspected. Existing report gaps are unchanged.
