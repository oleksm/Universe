# The planet studio, first step: every world in the store, looked at

- **WORLDS** (in the observer; `planet_studio`): the worlds store's index (`worlds::releases`:
  `worlds/releases.json`, every world the planet simulation has released, the game's or not: Grown
  Earth, Cinder, Harvest, Hearth's first survey), each with its radius, land, deposits, districts,
  surface version and its line about itself. ENTER goes to it: the observer round its body, its
  ground, air and clouds as the game draws them. L changes the look of a baked world's ground: its
  true colour, its rock (the bake's geology map), its oil and gas; its full maps are made again in
  it.
- Dev: the scenario `worlds` opens it gone to `UNIVERSE_WORLD` (a world id) in `UNIVERSE_LOOK`.
- Next: worlds that aren't the game's (Grown Earth), shown alone; the timeline of a world's growth
  (the lab's history frames, not in the bake yet).
