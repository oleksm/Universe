# Visual revamp, first pass (see docs/art-direction.md)

- **Render core:**
  - full resolution, 4× multisampled (no chunky low-res buffer); screenshots at 1920×1080;
  - the scene in HDR light, tone-mapped with a filmic curve (highlights roll off instead of
    clipping);
  - the HUD composited on top (crisp) until the interface pass.
- **Solid, not drawn:**
  - no contour outlines; edges are panel lines a little darker than the plating;
  - metal surfaces glint in the sun (a material per mesh: glint, sharpness, glow);
  - ships in liveries by trade (traders warm, pirates dark, miners ochre, shuttles pale; ours a
    clean grey-blue);
  - station and gate in metal greys;
  - planets without silhouette outlines;
  - planetshine paler (clouds and haze).
- **Lights** (a glow primitive: additive, facing the eye, a least size so far lamps still show):
  - ships' navigation lights: red port, green starboard, a white tail strobe on its own beat;
  - the station's pad lamps (the pad you're cleared for green), its window band, the hangar's
    light spill, red beacons;
  - gates' running lights: the entry side white, the exit side green, a pulse chasing round.
- **The station built out** (detail over its collision blocks):
  - a control tower over the deck, roof machinery, masts with cross-arms;
  - radiator fins, a rim round the deck, beams under it.
- **Ships:** engine bells at the mains (dark, heat-stained metal), glass canopies.
