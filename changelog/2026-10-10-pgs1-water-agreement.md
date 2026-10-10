# PGS1 water-domain reader agreement

- Added the upstream ocean-cut, lake-cut and spill-cap binary/query goldens as
  hashed regression fixtures: all 207 queries pass without reader changes.
- S15 surface_model_005 passes all 174 independent queries with exact ownership
  and water body IDs; terrain-height error stays below 1.41e-10 m. All nine PGS
  integration tests pass. Rendering/installation and S13 water remain separate.
