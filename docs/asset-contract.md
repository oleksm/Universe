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

Other views: `next_asset.py` (the brief as text), `--list 20` (the queue), `--summary` (records and models by kind), `--report` (done and left as a page, opened),
`--key <key>` (one record's brief), `--hulls` (hulls too).

## Packages: the contents convention

A package model is drawn full. What it carries is the meshes under one node named `contents`, all with one material
named `contents` (neutral light grey, no texture): the game tints that material by the stock's `tint` (sRGB 0 to 1 on
every stock record, artistic, tools/standards/tints.py), and may
hide or scale the node for a part-full package. Everything else (pallet, straps, cage, frame) is the package itself, in
its own materials, never tinted. No printed text in the model: its labels are blank plates (nodes `label`, `label_2`,
material `label`, 2:1, UV 0 to 1) that the game prints each packed unit's label on (docs/formats/freefall-label-1.md:
brand, product, lot, serial, where and when made and packed, weights, barcode). A closed package whose load cannot be seen (bulk bag, carton
pallet, cold crate, gas rack, heavy crate, tote, cryogenic dewar, livestock crate, fish tank) has no `contents` node and no hidden fill: it looks the same whatever it
carries, as real ones do. Where the load shows, it has one: the skip, produce crate, plate stack, bar bundle, coil, bale (the bale is its contents), ladle (the melt at the top). The contents stand for any stock the package carries: a
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

## Templates (kit families, 2026-10-17)

One model draws every size of a kit family (`standards/SFO/metadata/templates/`). Its brief is its **reference** record
(`reference:`, a member; `model_of` on the older templates is the installed size that serves as theirs).

- Build the model at the reference record's `physical` (a connector) or its pieces' sizes (a run's pieces), in metres,
  +Y up as every model. The size check is the usual one against that record.
- **Axes:** a connector's joint axis (the line its flow or current crosses) is +Y; the record's `height` lies along it,
  `length` along X, `width` along Z. The mating plane is y = 0, the model centred on the axis in X and Z.
- **Halves:** the two halves are named root nodes: `male` and `female` (a plug and its socket, a cam-lock's adapter and
  coupler), or `half-a` and `half-b` for androgynous ones; mated in the static pose. A valve or an expansion joint is one
  node, `body`, its flange faces at y = 0 and y = height. No motion.
- **Scaling:** the game draws each member at its own figures: X by length / reference length, Y by height / reference
  height, Z by width / reference width. A run's pieces (supports, trolleys) scale by its `outside` the same way.
- **Real shape first:** if the reference's figures are not what the real part measures, model the real part and send the
  measured sides; the registry changes the family's rule, never the model.
- A sweep template (hose, cable, conductor rail's housing) has no model: the game sweeps its profile.
- **Conductor rail and collector** (enclosed rail): the housing runs along +X, open in a slot underneath. Its section's
  datum is the slot mouth: y = 0 there, centred in Z, the housing rising to y = outside[1], `outside[0]` wide. The four
  conductors line the inside side walls, two a side, stacked: contact faces at z = ±(width/2 − 0.015), y = 0.3 and
  0.65 of the height (90 × 160 mm: z = ±0.030, y = 0.048 and 0.104; 120 × 200 mm: z = ±0.045, y = 0.060 and 0.130;
  invented, to be sourced from a maker's drawing). The collector is one root node `body` on the same datum: travel +X,
  its trolley above y = 0 inside the housing (no wider than width − 0.02), its four shoes facing outward ±Z onto those
  faces, its towing arm below y = 0 to the machine.
- **Corner casting** (ISO 1161): one root node `female`, a top corner of the right hand: the top face's outer plane at
  y = 0, the body below to y = -0.118; length 0.178 along X (the container's length), width 0.162 along Z; its open
  faces are the top, the end (+X) and the side (+Z), the game mirrors it for the other corners and flips it for the
  bottom ones. Apertures, from memory of the standard, provisional: top an oblong 124 x 63.5 mm with round ends, long
  along X, its centre 12.5 mm inboard (-X) and 8 mm inboard (-Z) of the casting's centre; end and side faces an oblong
  78 x 51 mm (78 across, 51 high), centred on their face 8 mm below its middle. Walls: top plate 26 mm (the twistlock's
  cone sits in it), the others 13 mm, hollow inside.
- **Panels** (building panels' templates): one root node `body`, static. A wall or partition stands with its foot at
  y = 0, its face along X centred on x = 0, its thickness along Z centred on the frame's grid line (z = 0), outside
  face toward +Z. A floor's walking plane is y = 0, the cassette below it; a roof sits on the frame's top, its underside
  at y = 0. A stair's foot is y = 0 at the lower floor, rising along +X. Doors and windows closed.
- **Furniture** (unit.furniture-*): one root node `body`, its feet at y = 0, centred in X and Z, its front toward -Z.

## Rework: what an installed model lacks, and redoing it

A record changes after its model is made. The rework tools say what that change asks of the model, so only real body
changes are remade (the owner, 2026-10-16: never the same model again without a good need).

    python3 tools/standards/next_asset.py --rework [--json]     # installed models with work to do, worst first
    python3 tools/standards/next_asset.py --gap <key>           # one model's gap: the spec it was made to, the spec now, what to do
    python3 tools/standards/next_asset.py --key <key>           # the full brief: size, parts, ports, motion, envelope, requests
    python3 tools/standards/model_measure.py <model.glb> --as=unit.<slug> [--list] [--nodes=a,b] [--apply]   # a unit's envelope boxes from its model

- **The spec** (`tools/standards/model_spec.py`): what a record sets for its model: size, part names, required nodes,
  ports (position, facing, connector), motion, envelope, triangle budget. Masses, prose and duties are not in it.
  Every install keeps the spec it was made to in its manifest (`spec`, `spec_sha256`); older installs are read from
  the record in git at the commit that installed them.
- **The verdicts**, worst first:

| Verdict | When | What to do |
|---|---|---|
| rebuild | the record's size left the model's tolerance, or the registry asks it by name | remake the body; install a new version |
| rig | parts of it move (a motion not of the whole unit) and the package has no `motion.json`, or a required node is missing | separate the moving parts, pivot them on the record's axes, install with `--motion` |
| review | the record's parts changed since (added, removed, renamed) | look: if the model shows them, nothing; if not, say so and remake |
| measure | a unit without envelope boxes, or the registry asks them measured | `model_measure.py`, its groups named so open space stays open |
| optimise | over its triangle budget | reduce, install a new version |
| none | a port moved or was added; a mass changed | nothing: the game draws the joint at each port, the model ends at the mating plane |

- **Requests by name**: a record's `model_requests` (`work`, `because`, `asked`, `of_version`, `by`). For plant units they
  are written from `tools/standards/unit_requests.yaml`. A request is open until a version above `of_version` is
  installed; a `measure` request closes when `model_measure.py --apply` writes the boxes.
- **What the registry does not ask**: a new model because a port moved, or a size within tolerance. A body conflict a
  layout can't route round comes as a request with its reason.

## Game validation

The registry validates installed models in the game. After `--push`, the modeller sends the registry a `task` naming the
key, the commit and the package's manifest. The registry then:

1. runs the game's own loader on every model (`cargo run -p universe-world --example validate_models`): the
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
