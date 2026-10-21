# PlanetSim ground catalogue contract and view ladder

Implements the owner’s catalogue specification (planet-forge 94e4c4f,
`docs/catalogue/README.md` §§1–3) as scaffolding for Planets’ record fill pass.

Replaces five context levels with eight view decades from 10,000 km to 1 m,
with target ground pixels at view/1000. Old references are mapped as agreed;
old sample counts are not reinterpreted as achieved rendering resolution.
The feature schema gains the realm/water/cover registers, marine/orographic/biotic
families, size/basis, derived ladder, ground description, shape laws/blend,
children, rule-selected growth, examples, settings and reciprocal line links.
Cover examples may cite a cover dataset instead of a DEM. Numeric feature
`level` is removed; existing context hints do not manufacture missing size data.

One audit implements six checks in the CLI, registry check and published report:
size/ladder, expansion references/order/cycles, examples and built geometry,
emitted-kind coverage, blend restrictions, and reciprocal line/feature meanings.
The 32-kind forge manifest is a versioned YAML snapshot; explicit manifest and
features.jsonl inputs can check newer source or run coverage without a sibling
checkout dependency. Range ordering, finite values, geographic bounds and
conditional example provenance are checked by validation.

This is deliberately migration mode: 175 existing kinds expose 937 gaps, mainly
missing size/description/shape/example data, plus the legacy trunk-river self-parent
cycle. No Earth sizes, examples or geological decisions are invented here.
Planets will fill records and remove that self-parent; Registry will enable strict
mode in a follow-up after the fill pass. `planet_catalogue.py --strict` already
rejects any gap. See `docs/planet-ground-catalogue.md` for the exact contract.

Validation: schema and reference checks, generators, stock/layout/port/mount
checks, engine registry reader, eight-level/feature/report pages pass. Thirty-two
regression tests pass, including twelve new catalogue tests for all six checks,
threshold boundaries, invalid values, context intervals, cover provenance and
strict rejection of an unregistered observed run kind. Forge manifest freshness
check passes. The prior 20 registry regressions remain included.
