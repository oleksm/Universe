# One engine for the game and the planet studio

*2026-10-06. Agreed with the user. The game's engine draws every world; the planet lab's three.js
viewer is retired into a planet studio inside the game. Studio first, the web build last.*

## The shape of it

- **One planet renderer** in the game's engine (`crates/engine`, wgpu): the globe and the near
  ground (cube-sphere patches), with true colour, sea, air, clouds, rivers and fine relief. Every
  world in the game is drawn by it.
- **One data contract:** a world's surface bake (`planet-sim-surface`, `docs/survey-contract.md`),
  extended with whatever the renderer reads. The lab bakes it, the registry records it, the engine
  reads it from the worlds store, each file checked by its hash (`world::worlds`). No other path.
- **A planet studio in the game** (beside the shipyard and interior studios): any world in the
  worlds store (Grown Earth, Cinder, Harvest, worlds not in the game), with the lab viewer's tools:
  the timeline of the world's growth, the layers (geology, oil and gas, peaks, weather), the
  parameter panels. The lab works in it; the game flies over the same code.
- **The web build, last:** the engine compiled to WebAssembly (WebGPU) inside the lab's page in
  place of the three.js canvas, so worlds stay shareable as links. It needs audio, threads and file
  loading made web-ready first.

## Who does what

| Who | Owns | Does |
|---|---|---|
| **Lab** (`planet-sim`, its own branch `planet` in `~/git/universe-planet`) | How a world looks, and its data | Writes the planet shaders in the engine, ported from its three.js ones (atmosphere scattering, sea, clouds, fine relief, rivers); bakes the files they need; gives reference views at fixed coordinates (`#v<lat>_<lon>_<km>`) as the target for each. |
| **Integrator** (`main`) | Engine plumbing, the game side, merges | Texture loading from a bake (large equirectangular maps, cube tiles, streamed in the background); the globe and near-ground renderer's shader slots; the studio's frame (opening a world, camera, timeline and panels in the HUD); the game's world and physics on the same data; checks each step frame by frame against the lab's viewer; merges `planet` when the user says. |
| **Scientist** (`fso`) | The contract on record | Writes the render package into `docs/survey-contract.md` and the bake record: which files, encodings, versions, hashes; the validator checks them. |
| **Spaceship Engineer** (`ships`) | (not affected) | Its studios run on the same engine and gain the shared light and air. |

## Order

1. **Contract** (Scientist): the render files as Harvest's bake holds them today: `globe_color`,
   `globe_normal`, `globe_spec`, `globe_clouds`, `climate`, `descent_color`, `atmosphere.json`,
   the `rv_*` river tiles, beside the heights already read (`hz_*`, `fz_*`).
2. **Plumbing** (integrator): a texture path for large maps and tiles; slots in the planet
   shaders. Proof: Harvest in its true colour, globe and near ground.
3. **Look** (lab): sea, air, clouds, rivers, fine relief, each checked against its own viewer at
   the same coordinates (both can be captured headless: the game with `UNIVERSE_SCREENSHOT`, the
   viewer with headless Chromium).
4. **Planet studio** (integrator, the lab's page as the model): any world from the store, the
   timeline, layers, panels; the lab moves to it and its three.js globe retires.
5. **Web build** (later): the engine to WebAssembly for the lab's page.

## Already in the game (2026-10-06)

- The worlds store and every package checked by its hash (`world::worlds`).
- Harvest's heights (5 km map and 600 m tiles) as its ground, for physics and drawing; the fine
  tiles streamed in the background for drawing (`worlds::Detail`).
- A body direction is the registry's latitude and longitude (north +Y, longitude `atan2(−z, x)`);
  the bake's cube tiles are in the body's own frame.
- Found: the viewer at `localhost:8080/harvest/` gives Harvest's highest point as 5,618 m; the
  bake installed in the registry gives 8,920 m. Which run does the viewer show?
