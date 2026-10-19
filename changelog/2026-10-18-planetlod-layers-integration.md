# Planet LOD viewer layers and pins

Merge planetlod-layers (41fdd5b9) into current main. The standalone planet LOD
viewer can load optional named labels, peak/dip pins, and transparent landform
layers with panel toggles and legends. Peak labels are limited to the nearest
300 visible pins. Existing views without these files retain their previous
behavior.

The four changes are isolated to crates/game/src/planet_lod_app.rs. The stopped
levels-lab branches are excluded. Integration validation uses the workspace test
suite and native game build; this merge does not claim a new visual playtest.
