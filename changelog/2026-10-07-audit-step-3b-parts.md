# 2026-10-07: registry audit, step 3b (i): parts as a rule

- 703 generated part files in 146 equipment folders are gone; each equipment record carries its
  parts as `built_of.list` (code, name, mass, box, item, quantity, module; a basis only where the
  derived one would differ; `shares` and `whole` as folder-wide defaults). The build and the
  registry reader derive the part records from the list through one template
  (`tools/standards/lib.py` part_yaml, `crates/registry` derived_part_yaml), so `reg.parts`, the
  game's items and marks, the chains and the Mass report see what they saw. Checked field for field
  against the files before deletion (`tools/standards/parts_inline.py --check`: 0 differences, basis
  included). Three more rules (shares chosen, haul-truck shares, built in whole). Measured parts
  (the MC-07's and the gate's) and the 32 module part folders keep their files.
- Step 3b (ii), bulk stock derived from goods, is dropped: it would need a good to name its
  market, against the user's rule that only stock is sold.
