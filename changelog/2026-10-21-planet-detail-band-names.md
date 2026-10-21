# PlanetSim detail bands named by upper bound

Renames the eight level keys to `planet-level.10000-km`, `1000-km`, `100-km`,
`10-km`, `1-km`, `100-m`, `10-m`, and `1-m`. Names read “100 km band”; descriptions
state the interval down to the next decade. The 1 m band is explicitly texture
only. This is the owner’s naming revision relayed by Planets after the catalogue
scaffold shipped; it changes no view/pixel targets or ground sizing rules.

Every live metadata reference, context example and derived-ladder key uses the
new names. `standards/renames.yaml` adds twelve direct aliases: the eight replaced
names plus cluster, pattern, square and detail from the original context ladder.
The level register retains its coarse-to-fine view order. Ladder means
[coarsest band, finest band]. The later lower-view/170 mesh-spacing guideline
remains a forge guideline, not a registry rule, as Planets confirmed.

Validation: 33 registry regressions pass, including saved-key aliases resolving
to live bands with the agreed view bounds. Full registry and generated-module
checks, engine reader, band register and catalogue pages pass. The 937 catalogue
migration gaps remain unchanged; Planets fills records after this delivery.
