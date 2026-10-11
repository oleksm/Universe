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

## Packages: the contents convention

A package model is drawn full. What it carries is the meshes under one node named `contents`, all with one material
named `contents` (neutral light grey, no texture): the game tints that material by the stock's `tint` (sRGB 0 to 1 on
every stock record, artistic, tools/standards/tints.py), and may
hide or scale the node for a part-full package. Everything else (pallet, straps, cage, frame) is the package itself, in
its own materials, never tinted. No labels, no text. The contents stand for any stock the package carries: a
representative load, not its count, cross-section or weight. The bar bundle's contents are round tubes (71 of the 73
stock records it carries are tubes).

## Static models

The game draws a registry model as it stands: no animation clips, skins or deforming meshes (hoses, bellows). A part
that will move (a gimbal, a door, a gear leg) is its own rigid mesh under its own node with an unambiguous name (no name
inside another's), and the handoff gives its parent, bind transform, pivot and axes in model-root coordinates (metres,
+Y up): the loader bakes node transforms into the vertices, so these are not read from the model. What the game does with such parts, and what Blender hands over for a
moving engine: engine's `docs/ships/ch-s2-motion-contract.md` (main 462a7b45).

## The queue

Kinds that take a model: equipment, packages (the standard units stock travels and lies in: `standards/SFO/metadata/packages/`, one model each, drawn for every stock that names it; `tools/standards/packages.py` assigns them), industrial modules, buildings, structures, gate rings, and hulls on asking. Stock itself takes no model: it is drawn as its packages. A record
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
| Size | the model's three sides (world bounds of its meshes), sorted, each 85% to 110% of the record's three sides, sorted, and 5 cm either way for thin things. A flat thing (its thin side under a fiftieth of its longest: a field, a pad) has its thin side held only to at most the record's. The registry is the size authority: if the record is wrong, send a request to the registry; never fudge the model |
| Nodes | equipment: `mount*` (the face it attaches by); jet engines `nozzle*`; ducted fans `duct*`; landing gear `contact*`; doors, ramps, lifts `door*`; docking `dock*`. Other kinds: none required yet |
| Collision | meshes named `COL_*` are collision; they are not counted as drawn |
| Budget | triangles drawn: equipment 60,000; modules and buildings 150,000; structures and gates 400,000; hulls 1,000,000 |

## The package

In the assets store (`UNIVERSE_ASSETS`, else `~/git/freefall-assets`): `models/<key tail>/v<N>/` holding `model.glb`
and `manifest.json`: format `freefall-model/1`, key, kind, version, `bounds_m` (the model's world bounds as [[min x, y, z], [max x, y, z]] in glTF metres: the game centres the model on its thing's place with it), `size_m` (its three sides), the record's size it was checked
against, its node names, triangles, source (the .blend's path and sha256, its repository commit, the exported file),
date, note, and `files` with sha256 and size. A version is written once; a new export is a new version. The installer
reads the model itself, so the modeller writes no manifest.

## The record

The record's `visual` (common.schema.yaml): `{format: freefall-model/1, path: models/<tail>/v<N>, manifest_sha256,
version, triangles}`. The installer writes it, runs the validators and the registry build, puts the record back if
either fails, and with `--push` commits it on fso. Running it again with the same file changes nothing.

## Installing: one command

From `~/git/universe-fso` (branch fso, no unrelated changes staged):

    python3 tools/standards/next_asset.py --key <key> --json          # the brief
    python3 tools/standards/install_model.py /abs/model.glb --as=<key> --source=/abs/model.blend --about=/abs/about.yaml          # dry run
    python3 tools/standards/install_model.py /abs/model.glb --as=<key> --source=/abs/model.blend --about=/abs/about.yaml \
        --thumb=/abs/thumb.png --icon=/abs/icon.png --push                                                          # install
    (add --motion=/abs/motion.json for a model whose parts move)

Inputs: the exported `.glb` (metres, static: no animations, skins or morph targets), its `.blend`, two previews
(`--thumb=thumb.png`, 512 x 512 px, and `--icon=icon.png`, 128 x 128 px: square PNG with a transparent background, the
item alone in a three-quarter view, rendered from the same source; required with `--push`, written into the package
as `thumb.png` and `icon.png` and listed in the manifest's `previews` and `files`), and the modeller's
account (`--about`, YAML or JSON; template `docs/asset-about.example.yaml`): `model` (key, purpose, source revision,
components, basis, interfaces, motion scope), `work` (what this version built or changed; runtime versus source-only),
`considerations` (decisions, easements, assumptions, limitations, defects, each marked), optionally `stats` (what the
installer cannot measure: texture memory, draw calls; unknown is `unknown`) and `evidence` (hashes, QC, approval).
`--push` refuses without `--about`. The account goes into the manifest as `about`, beside `measured` (the installer's
own count: triangles, vertices, meshes, primitives, materials, textures, images, nodes, animations, skins, morph
targets, bytes). The same model with the same account again changes nothing; a changed account is a new version.

## Moving parts: `freefall-motion/1`

A model whose parts move carries `motion.json` beside `model.glb`: `--motion=<file.json>` to the installer. Format:
`docs/formats/freefall-motion-1.schema.yaml` (moving nodes with parents and neutral binds in model-root space; the
gimbal's pivot, axes, nesting, normalisation and limit; actuators by their two anchors; hoses by end frames and
tangents, diameter, free length and tolerance, bend radius and its source, neutral centreline, guides; rigid links
between two joints: one pose producer each, `inherited` (rides a listed parent) or `solved` (placed from joint a toward
joint b, rolled by a stated rule), joints framed on both sides with ordered hinges, limits and their source, axial,
lateral and twist allowances, tie rods; bellows as rigid rings on the Hermite curve between two cuffs, the evaluator
engine and blender agreed for v04). The installer checks it against the schema, the model's nodes, rigid binds,
parents, the record's gimbal limit, actuator overlap and hose reach (16 directions), and over 65 poses: link closure,
each joint's relative rotation decomposed in hinge order (hinge angles against limits, twist and slide against
allowances, any other turn refused), roll never undetermined, tie lengths, bellows rims against their skirt, and every
reference pose's node transforms within 2e-6; then writes it into the package (listed in the manifest's `files`). The game does not read it yet (engine's sequence, docs/ships/ch-s2-motion-contract.md on main).

## Pre-acceptance check: one command, by the modeller

    python3 tools/standards/precheck_model.py /abs/candidate/delivery.yaml

From `~/git/universe-fso`. It checks the delivery record's `hashes`, that its `thumb` and `icon` are there and fit
(512 and 128 px, square PNG, transparent), runs the installer's dry run with `--source`,
`--about` and `--motion`, and, with a motion file, the game's own CPU consumer (engine's `ch_s2_fixture` example: the real
model loader, every reference pose, a 513-pose sweep), then writes `review/registry-precheck.md` beside the record.
Exit 0 is a pass: no registry review is needed before the owner's acceptance. Exit 1: send the registry a task with the
report. The first build takes a few minutes; later runs are quick.

## Game validation

The registry validates installed models in the game. After `--push`, the modeller sends the registry a `task` naming the
key, the commit and the package's manifest. The registry then:

1. runs the game's own loader on every model (`cargo test -p universe-world --test visuals -- --nocapture`): the
   package found, its manifest matching the record's hash, the model matching the manifest, bounds to centre it by;
2. where the game draws that kind, looks at it in the game (frames captured on workspace 8): place, scale, materials,
   readability at play distance;
3. records the result in `docs/asset-validation.yaml` (key, version, manifest sha256, game commit, result, what was
   checked, evidence) and answers the task with `done` (pass) or `blocked` (what fails, whose fix).

The package is never edited after installing: a fix is a new version.

## Open

- The game draws models for settlement modules and buildings, rigs and gates (main 364a539e); equipment at mounts and
  stations are not drawn yet (engine's), so their validation is step 1 only until then.
- Module and building nodes (entrances, ports, pipe connections) when the
  game places things by them.
- Previews for the models installed before 2026-10-10's preview rule: reinstall the same `.glb` with `--thumb` and
  `--icon` (a new version; the model is unchanged).
