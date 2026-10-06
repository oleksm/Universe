# 2026-10-06: the survey contract

- **A body can be baked:** `provenance: baked` (dictionary), a `survey` section on the body schema
  (world ID, folder, manifest hash, simulator commit, headline figures, the endowment in kg).
- **`tools/standards/survey_import.py`:** checks a simulation's survey against its manifest, copies it
  read-only to `standards/Celestial/surveys/<world_id>/`, points the body at it. Refuses a world not
  grown from the record (radius), a different survey under a known ID, or re-baking without
  `--replace`.
- **`docs/survey-contract.md`:** what the simulation reads from a record, what the import does, what
  the game reads from the survey and for what, the later layers (air, weather, foliage, animals).
