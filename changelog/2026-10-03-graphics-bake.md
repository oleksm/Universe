# Graphics settings; baked materials; ambient occlusion

- **Graphics settings** (` opens; 1-8 switch, the scene live behind the panel; kept in
  `graphics.json` beside the quicksave): shadows, model textures, normal maps, ambient
  occlusion, lamps and glows, glossy highlights, planet light, the film curve (AgX). Each off
  alone, to see what it brings and trace a difference in looks to it.
  `UNIVERSE_GRAPHICS=-shadows,-occlusion` for captures.
- **Ambient occlusion on models:** glTF occlusion maps read; they dim the light from round
  about (the ambient floor, the planet's light and its reflection), never the sun's.
- **`export_hull.py --bake 4096`:** node-made materials (glTF can't hold them: they came out
  plain white) baked into one atlas the meshes share: base colour, roughness, metalness,
  normal, ambient occlusion. Lamps, glows and glass keep their own materials. The game's
  collision boxes are kept out of the light while baking.
- `UNIVERSE_SHADOW_DEBUG` tints models in shadow red too (it showed the MC-07 on the station
  deck in the deck's shadow: the sun below the deck plane, not a fault).
- Dev: `sunlit` (our ship in open space, the sun over the eye's shoulder), for comparing looks.
