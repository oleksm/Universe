# PGS1 diagnostic rock-colour preview

`UNIVERSE_PGS1_COLOURS=categories` enables the export's `surface.json`
`categories.legend` palette in the explicit PGS preview. Default `neutral`
keeps the comparison view. The legend is checked against the manifest, surface
hash, vocabulary names and source-to-ID mapping; it is indexed by frozen rock
ID, never source-array position. Unknown, unassigned and unmapped labels remain
neutral. Known water keeps precedence. Patterns are retained but not coloured.

Colours are diagnostic, not physical textures. Physics and canonical queries
are unchanged. Rendering samples the legend into the existing 512-square cube
map and filters it at draw time, so boundaries/very small regions remain limited
by map resolution. No installation, registry edit or main merge.

Validation: workspace tests (including shader validation), palette parsing and
unknown-ID regressions, player check and release build pass. Inspected Vulkan
colour/neutral orbit comparison and colour-mode lake/shore view.
