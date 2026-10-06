# Harvest's fine ground read in the background for drawing

- **Detail levels** (`worlds::Detail`): the physics reads a world's 600 m tiles in full (waiting for
  a tile it lacks: what it stands on is the same every run); drawing reads them as far as they're
  read (`Terrain::surface_view`), any missing asked for in the background, and a patch made before
  its ground was all read is made again once it is (`terrain_lod`); a whole globe's map reads the 5
  km heights alone (`height_and_crater_coarse`), where it used to read all 635 tiles (1.3 GB).
- Low over Harvest: 144 frames a second, the worst of 300 at 10 ms, no hitches.
- (Yesterday's note of the first frames waiting on tiles: that was the globe's map reading every
  tile; the bar at the top left is the hypernet's lag, not the frame time.)
