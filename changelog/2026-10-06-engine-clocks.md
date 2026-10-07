# 2026-10-06: the Engine root and its clocks

- New root **`standards/Engine`**: how the engine governs itself. Schema `clock.schema.yaml`
  (parent, scope, coupling, trigger, runs, reads, posts, rule); dictionary words `clock_scope`,
  `coupling`, `trigger_kind`. Eleven clocks under `metadata/scheduling/` (a twelfth, Administration, the same night): Galaxy (1 day), Region
  (events), Star system (group), Celestial (60 s), Planetary (60 s, per body), Economy (10 s, per
  settlement), Market (1 s), Machinery (1 s, per craft), Rails (events), NPC lane (1 s), Realtime
  (5 ms, per bubble, tight). Periods are the user's and are configuration: the engine reads them
  here. The build's **Clocks** report draws the tree and checks every period is a whole number of
  the realtime tick. `docs/tick-tree.md` rewritten to this tree.
