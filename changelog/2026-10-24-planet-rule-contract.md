# Planet rules: explicit evidence and canonical catalogue mappings

Patterns now support required gates, inclusive SI conditions, raw-unit soft bands, directed categorical membership,
and per-form decay ages. Missing/nonfinite observations, unresolved classes and unavailable drivers stay unknown.
Required gates do not contribute weight, and an empty gate-only vector cannot place a feature. The generated
Python matcher shares this logic. Historical layer op/value classification remains separate from new pattern rules.

Reviewed all 62 factor readings against the current producer inventory. Removed false aliases from thickness to ice
flow, annual means to seasonality, per-step deposition to sediment supply/regolith, and crust age to landform age.
Retained model proxies and arbitrary numerical calibrations remain explicitly reviewable. Raw rock codes and frozen
vocabulary IDs are distinct; ambiguous fine/coarse clastics and absent evaporite evidence never become known absence.

Corrected floodplain discharge, meandering slope, lava-flow-field slope and ria ice preferences. Merged three duplicate
vectors while preserving historical keys, sources and vocabulary IDs. There are 261 canonical patterns mapping all
241 feature kinds, including 79 rules explicitly owned by feature triggers. Such trigger-owned rules require record
feature evidence; their empty scoring vectors cannot fabricate occurrences. Reviewed categorical prerequisites and
modifiers separately, with 53 necessary gates and unresolved intentions retained. Solutional-karst lithology gates
have a USGS source; purity, thickness and groundwater remain independent evidence gaps.

Verified seamount relief, ice-body area and permafrost-duration conventions against primary sources. Source details
are in `standards/sources/research_planet_rules_20261024.json`. Corrected the recent river-change timestamp to the
producer's formation clock. Two silent range-crest fallback values are now explicit invented/review process parameters;
existing process values are unchanged.

The earlier count of 157 comparisons could not be reproduced. The exact upstream inventory has 448 literal rows:
102 physical gate values, 41 categorical codes, 119 lattice values, 41 model constants, 5 shaping/geometry values,
136 numerical guards and 4 dead-code values. `docs/planet-pattern-m3-threshold-review.yaml` records their disposition.
They are not all scientific thresholds. The 180 canonical TIME entries await defensible per-form decay constants;
formation-duration prose is not silently converted to relaxation time.

Validation: 89 registry unit tests pass, including 14 pattern semantics tests. The registry validator reports zero
faults. Final registry build/module/catalogue/fit/maker/supply/stock/port/mount and detail-page checks are recorded in
the delivery audit. The old registry_e3 simulation snapshot remains untouched; registry_e4 is a separate snapshot.
This delivers the rule contract and coverage, not completed M3 terrain placement or scientific calibration. Missing
classification/history drivers and the consumer migration remain explicit work for the coordinated M3/M4 lanes.
