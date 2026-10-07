# 2026-10-07: registry audit, step 3b (iv): one naming rule for equipment

- Every equipment key reads `equipment.<slot>[.<family>].<variant>`: the slot's word, the design's
  family where it is not the slot's own word, and the variant (s1, 5t, crew, mc07). 74 keys
  changed (`equipment.tank.water.tank.s1` → `equipment.tank.water.s1`, `equipment.rack.s2` →
  `equipment.cargo.rack.s2`, `equipment.bay.hopper.5t` → `equipment.cargo.ore-bay.5t`,
  `equipment.life.s1` → `equipment.life-support.s1`, `equipment.store.food.s1` →
  `equipment.cargo.food-store.s1`); files and parts folders are named after the key's tail
  (`tank-deuterium-s1.yaml`). Every old key is in `content/base/aliases.ron`; prices, hull fits,
  mounts, docs and three crate literals follow (`tools/standards/rename_equipment.py`).
