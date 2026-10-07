# The registry by key (audit phase 3)

- The generator gives every record kind a lookup by key on the registry (`reg.hull(key)`,
  `reg.part(key)`, `reg.body(key)`, ...), through an index of each kind's records built once when
  the registry is read or decoded (`KeyIndex`, not serialized). The hand-kept `good`, `module` and
  `system` lookups are generated now.
- 44 searches of a kind's list by key replaced (in world, services, sim, the game, the registry
  itself); the "outdated hull" check written four times is one (`ship::outdated_hull`), and "the
  hull record of a game hull (by key, else by its model)" written twice is one
  (`ship::hull_record`). What's left comparing keys isn't a registry search (a works' setups, a
  need's key).
- The economy step: 1.23 ms (1.18 before; those searches weren't on its hot path).
