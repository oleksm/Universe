# Walker: one push per surface, not one per triangle; a record's empty slots stay empty

Ships `20261024T101619-ships-7f03` (an 11.4 m jump boarding the Explorer V2) and `20261024T102258-ships-205e`.

- `Walker::step` pushed the body out of every overlapping triangle in turn, with depths measured once: a surface
  made of many triangles summed its overlap (the ships' repro: 10 mm overlap moved 10 mm with 2 triangles, 320 mm
  with 64). Now the deepest contact per probe is applied, then contacts are asked again (up to 8 rounds): a surface
  pushes once, corners still resolve. Test: `walk::tests::walker_push_out_ignores_triangle_count`.
- `import`: a registered record's slots are used as they are, none included (a bare ground stage). Standard slots
  are inferred only for a model no record describes. Test: in `import_registered_model_without_nozzles_is_a_stage`.
- The Explorer V3 stage check re-run with both: `docs/ships/explorer-v3-stage-check.txt` (unchanged results).
