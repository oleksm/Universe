# 2026-10-06: the survey contract

- **A body can be baked:** `provenance: baked` (dictionary), a `survey` section on the body schema
  (world ID, folder, manifest hash, simulator commit, headline figures, the endowment in kg).
- **`tools/standards/survey_import.py`:** checks a simulation's survey against its manifest, copies it
  read-only to `standards/Celestial/surveys/<world_id>/`, points the body at it. Refuses a world not
  grown from the record (radius), a different survey under a known ID, or re-baking without
  `--replace`.
- **`docs/survey-contract.md`:** what the simulation reads from a record, what the import does, what
  the game reads from the survey and for what, the later layers (air, weather, foliage, animals).

# 2026-10-06: planets named, and matched by ID

- **Names:** the planets of Treistun are named by their people: Cinder, Anvil, Harvest (d, the
  farm world), Hearth (e, the home world), Rime, Drum, Pale, Hush, Verge; moons follow their
  planet. `identity.also` keeps the seed's letter. 28 records reworded.
- **The body's key is its ID.** The seed's bodies carry `key` (`system::Body::key`, slugged from
  the seed's name once, `StarSystem::key_bodies`); the registry's records match by key, not name,
  and a matched body takes the record's name (`celestial::apply`). Settlements find their body by
  key. A rematch test over the charted region: 109 records matched by key, 24 of them renamed.
- **Seedable inputs** a planet run wants, on the body schema: `interior.radiogenic`,
  `water.mass_share` (and `physical.age` on the star, which existed).
