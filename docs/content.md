# Content

Everything the world is made *of* — kinds of goods, recipes, places, hulls, modules, brands,
licences, shapes — is **content**: data, not code. The code knows behaviours (what a thruster
does, how a market prices, how a recipe runs); the content says which thrusters, markets and
recipes exist and with what numbers. This is what lets the game grow to hundreds of modules,
hulls and brands, from files (and later from Blender), without the engine changing.

## Packs and the registry

- Content lives in **packs**: a folder of RON files (`content/base/` is the game's own). The base
  pack is built into the binary, so every build and test has it; **override packs** (folders
  named by `UNIVERSE_CONTENT`, in order) add entries or replace them by key. Modding is an
  override pack.
- Loaded once at start into the **registry** (`world::content::Content`), immutable and shared
  (`Arc`) by the core, the services and the clients alike. Hot loops never see strings: entries
  are reached by **handles** (a typed index), resolved from keys at load.
- Every entry has a **key**, readable and stable: `hull.cobra`, `goods.fuel`, `recipe.refinery`,
  `drive.kestrel.k2`. Keys are what's stored and sent — in saves, the ledger and the protocol —
  never list positions, so adding or reordering content breaks nothing. Renamed entries keep
  working through an **alias** table in the pack.
- Generated content (the seeded goods catalogue, star systems) stays procedural — same seed, same
  galaxy — but generators read their tables (word lists, price and mass ranges) from content,
  and what they generate gets keys derived from the seed.

## Validation and the content hash

- At load every reference must resolve, and the physics must be sane (positive masses, thrusters
  that push somewhere, modules that fit their slots, hulls that take the base blocks, shapes
  that match their hulls). Bad content fails loudly at start, never mid-game. A fast test runs
  the validator over the real packs.
- The loaded content has one **hash** (over the packs' sources, in load order). Saves record it;
  multiplayer peers compare it. A mismatch is reported, not discovered as nonsense.

## Saves

Saves carry a **version** and the content hash. Loading runs the migrations from the save's
version up, step by step, and maps keys through the aliases. Old saves keep working as content
and code grow.

## Shapes: geometry the physics uses

A **shape** is one geometry asset that everything reads — not a picture with numbers kept
elsewhere. Today's shapes are built in code; later they come from Blender as glTF 2.0 (`.glb`).
Both produce the same in-memory `Shape`. Conventions: metres, −Z forward, +Y up (glTF's own; the
Blender exporter converts), nodes named by role:

| In the shape | Used for |
|---|---|
| render mesh | the wireframe (edges by crease angle: the vector look stays) |
| `col_*` meshes (default: the convex hull) | collision, as polytopes |
| parts with a mass or a material density | mass, centre of mass, inertia tensor |
| `mount_<slot>` empties | where modules attach (position, orientation, size), and their visuals |
| `nozzle_*` empties | thrusters built into the hull: direction and lever arm |
| `gear_*` empties | landing contact points |
| `dock_*`, `cockpit` | docking alignment, the cockpit camera |

Derived from the shape: drag area by direction (its silhouette), skin area (heating), bounds
(validated against the pads and docking slots of its size class). Validation checks that a
shape's mounts and nozzles match its hull's definition.

## Ships

A ship is a **hull** (a shape, structure, slots) and a **fit** (a module in each slot). Its numbers
— mass and inertia, thrust envelope, tank, hold — are derived from the hull and the fit, never
set by hand, and cached per distinct fit (identical fits are shared, so a hundred thousand ships
cost little). See the ships plan: base blocks every ship must carry, specialised modules, placed
thrusters that define how it handles, brands.

## The order (C0)

1. The registry: packs, keys, handles, validation, hash. The Cobra's hull into it.
2. Recipes and kinds of place into data.
3. Kinds of goods into data; the generator reads its tables from content; goods keyed.
4. Saves: version, content hash, keys for goods and hulls.
5. Shapes: the `Shape` type (render, collision, mass, mounts, nozzles, gear); today's shapes
   into it; a slot for the glTF loader.

Each step leaves the game as it was — same seed, same galaxy, same numbers — with the tests green.
All five are done.

**Known gap, closed in T1:** a ship's collision is still its hull's sphere (`radius`, 12 m for the
Cobra), while its shape is 52 m across the wings. T1 makes the shape the collider and derives the
mass properties from it.
