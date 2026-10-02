# Landscapes

- **Terrain with small-scale relief (physics):** eight more octaves of half-ridged noise from about
  25 km features down to about 70 m, with the same slope at every scale. Rough in the mountains,
  gentle on lowlands, dying down on a plain round each spaceport (none within 20 km, all of it by
  60 km). Collisions and landing use it: what you see is what you land on. The height bound counts
  it exactly (each octave stands at most 0.3 of its amplitude above zero).
- **The ground as a quadtree** (`terrain_lod`): below `near_altitude` a world is a cube-sphere of
  16×16 patches of that very height, splitting toward the eye down to about 30 m spacing.
  - Patches are made a dozen a frame on all cores, kept 600 frames, the parent standing in till its
    children are ready, and culled behind the horizon.
  - Each has its own origin (no shake up close) and skirts hide the seams between sizes.
  - Replaces the old flat ground patch; F4 still lays its grid lines over the ground.
- **Shaded as from orbit:** patches use the globe's per-pixel surfacing. Up close, coast and biome
  heights come from the patch's own geometry (a beach exactly where the ground meets the sea); the
  sea's depth comes from the map.
- **No speckle up close:** the fine detail's pixel size is measured in the eye's metres (precise)
  with the patch's exact radius, and the detail stops at about 100 m, where the mesh takes over.
- **Landing approach:** till over a port's 20 km plain, aim high over the port (above the tallest
  ground), not straight at the entry point; the hills short of the plain stood in the way.
- **Galaxy map:** opens with its scale bar at 50 ly. It was sized from the screen's pixels but drawn
  in half-size HUD units, so it opened at about 5.
- `noon`/`dusk` scenarios take `UNIVERSE_SUN`, `UNIVERSE_ALT`, `UNIVERSE_DOWN`; the moon's outcrops are softer.
