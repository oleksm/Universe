# Reviewing a new hull import

`hull_import_review` compares the production GLB importer result with the registry
embedded in its binary. It does not commission a handle or write a save. It reports
missing, added and changed slots, equipment instances and nozzle links, including
duplicates and nozzle share changes. It also compares effective frame/dry mass
with registry mass and the complete intended fit, and reports inactive nozzles,
missing mount markers and importer refusals.
Per-instance consumer rows distinguish unavailable products from inert modules,
and expose nominal registry needs/function versus native power draw/function.
An inert command or gear instance retains mass without implementing its operation.

From the game worktree root, with the registry and consumer changes integrated:

```sh
/home/alexm/bin/capped cargo run -q -p universe-world --example hull_import_review -- \
  assets/models/explorer-study.glb hull.explorer-study > /absolute/review/import.json
```

Use the **exact `hull.model` string**, including relative-path spelling. The
importer binds records by string equality; the diagnostic records both that
argument and its canonical path. An absolute spelling of the same bytes can
otherwise lose the registry binding and enter the inferred-fit route. Registry
data is embedded at build time; rebuild after records change. The JSON pins the
model, encoded registry and compiled importer/mass/fit source hashes. Record the
tested repository commit alongside the output.

Exit status 0 means the listed comparisons are clear; 2 means a discrepancy or
unresolved input. JSON is still emitted for an importer refusal. Invalid CLI or
unreadable inputs return an ordinary error. Inspect every reported fault. A clear
comparison is not flight, load, pressure, human-clearance or motion acceptance.

The diagnostic uses `goods::hull_mass` (introduced in `ed70fa59b`): a declared hull
mass first, otherwise a complete recursive parts total. Missing part masses do
not silently count as zero. It cannot establish
that a declared mass matches the mesh, or that equipment was not baked into the
hull: those require composition/BOM and export-ownership evidence. Equipment mass
is counted once per registry fit entry, including repeated products in different
slots. Fuel, cargo and crew are separate from dry mass.

For Explorer production-v01, `hull.explorer-study` and
`assets/models/explorer-study.glb` are reserved proposals. No placeholder record or
model is created by this tool. Blender must supply the measured hull-only export,
its hash and intended instance roster. Registry supplies the real record and
composition/mass evidence. Engine owns native command/gear retention and importer
fallback changes; Ships owns this comparison and its exact-candidate result.

The immutable visual package and `hull.model` exposure must contain identical GLB
bytes. Once installed, verify the package's manifest/model hashes and the exposed
game path against the reviewed export; record all three hashes. `hull.visual`
installation alone does not establish the `hull.model` binding. Do not change the
model path argument just to make a different file resolve.

Until those inputs exist, the standalone showcase can review an assembly GLB:

```sh
env UNIVERSE_TEST=1 UNIVERSE_SCENARIO=showcase \
  UNIVERSE_MODEL=/absolute/review/assembly.glb UNIVERSE_MODEL_CLEAN=1 \
  target/release/freefall
```

Use a scratch `XDG_DATA_HOME` for review. This previews the supplied model; it does
not commission that model. Normal world/player initialization still occurs.
Keep equipment reference instances distinct in the review assembly, and exclude
them from the hull-only export. `UNIVERSE_HULL` and hull-bound Studio import both
invoke physical commissioning and are separate gates.
