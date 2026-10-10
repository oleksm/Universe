# Asset contract: registry records to models, and back to the game

2026-10-10. How the Blender agent finds the next record that needs a model, makes it, and installs it, by command
line alone. The registry is the inventory: every record of a kind that takes a model either has a `visual` pointer or
is in the queue. Until it has one, the game draws it as a translucent box of the record's size with its name on the
edges, so nothing waits on a model to exist.

## Who owns what

| Owner | Owns |
|---|---|
| Registry (fso) | keys, sizes, the required nodes and budgets, this contract, the two commands |
| Blender agent | the model files and their sources |
| Ships session | hull designs (hulls are queued only on `--hulls`) |
| Integrator (main) | the game's loader for `visual` packages |

## The loop

    cd ~/git/universe-fso
    python3 tools/standards/next_asset.py --json                     # 1. the next record wanting a model, as a brief
    # 2. model it in Blender to the brief; export glTF binary (.glb), metres, +Y up
    python3 tools/standards/install_model.py model.glb --as=<key> --source=model.blend            # 3. dry run: the checks
    python3 tools/standards/install_model.py model.glb --as=<key> --source=model.blend --push     # 4. install, commit, push

Other views: `next_asset.py` (the brief as text), `--list 20` (the queue), `--summary` (records and models by kind),
`--key <key>` (one record's brief), `--hulls` (hulls too).

## The queue

Kinds that take a model: equipment, industrial modules, buildings, structures, gate rings, and hulls on asking. A record
is queued when it has no `visual` and is not retired, in the order the player sees things most:

1. equipment fitted on hulls and structures, most fitted first;
2. structures and gate rings;
3. industrial modules standing at facilities, most built first;
4. buildings standing at settlements, most built first;
5. the rest; within each tier current designs before outdated ones.

Not queued, by design: goods and stock (drawn by their form: a few containers will serve them, a later step),
generated parts (they are nodes inside their product's model), planets and their ground (the planet pipeline's, through
`install_world.py`), asteroids (procedural).

## The brief (`next_asset.py --json`)

`key`, `kind`, `name`, `description`, `maker`, `design_stage`, `used` (how many stand in the world), `size_m`
(length, width, height from the record), `mass_kg`, `function` (what the device does, its figures), `mount` (for
equipment: the mount it fits, its envelope and attachment points), `parts` (code, name, mass, box: what the thing is
built of, to lay it out), `required_nodes`, `triangle_budget`, `record` (the YAML file), `install` (the command), and
`package` (where it will land).

## What a model must satisfy

| Check | Rule |
|---|---|
| Format | glTF 2 binary (`.glb`), metres. Axes are free for the size check; equipment and hulls follow docs/ship-import.md (+Y up in the export) |
| Size | the model's three sides (world bounds of its meshes), sorted, each 85% to 110% of the record's three sides, sorted, and 5 cm either way for thin things. The registry is the size authority: if the record is wrong, send a request to the registry; never fudge the model |
| Nodes | equipment: `mount*` (the face it attaches by); jet engines `nozzle*`; ducted fans `duct*`; landing gear `contact*`; doors, ramps, lifts `door*`; docking `dock*`. Other kinds: none required yet |
| Collision | meshes named `COL_*` are collision; they are not counted as drawn |
| Budget | triangles drawn: equipment 60,000; modules and buildings 150,000; structures and gates 400,000; hulls 1,000,000 |

## The package

In the assets store (`UNIVERSE_ASSETS`, else `~/git/freefall-assets`): `models/<key tail>/v<N>/` holding `model.glb`
and `manifest.json`: format `freefall-model/1`, key, kind, version, the model's bounds, the record's size it was checked
against, its node names, triangles, source (the .blend's path and sha256, its repository commit, the exported file),
date, note, and `files` with sha256 and size. A version is written once; a new export is a new version. The installer
reads the model itself, so the modeller writes no manifest.

## The record

The record's `visual` (common.schema.yaml): `{format: freefall-model/1, path: models/<tail>/v<N>, manifest_sha256,
version, triangles}`. The installer writes it, runs the validators and the registry build, puts the record back if
either fails, and with `--push` commits it on fso. Running it again with the same file changes nothing.

## Open

- The game's loader for `visual` packages (the integrator's): read the package from the assets store, check its manifest
  sha256, draw it at the record's place; the box fallback stays for records without one.
- Containers for goods and stock by form; module and building nodes (entrances, ports, pipe connections) when the
  game places things by them.
