# Fitted equipment in flight and the observer studio

Owner request relayed by ships `20261010T194153-ships-bb43`. The shared hull
render path now resolves installed visuals from the current `Ship::spec().fit`.
It does not equip the catalogue. The registry visual and package hashes are
checked by the existing asset loader; packages are cached after first use.

Placement follows fso `docs/equipment-mounting.md`: accumulated hull glTF
`mount_<slot>` frame times inverse equipment `mount_<slot>`, otherwise `mount`,
otherwise model-frame identity. Preserve full authored roll and ancestor
transforms. No bounds centring, guessed look direction, or rescaling. Subtract
hull COM once, then apply ship position/orientation. Nonrigid mounts, duplicate
node names and animated/skinned/morphed or motion-contract assets are refused by
this static consumer. Missing/failed models or sockets retain native drawing.

The current reviewed development subset uses ships' hash-bound overlay:
`../blender/mc07-static-fit/mount-overlay.json`, reviewed in `fit-review.yaml`.
It adds equipment.computer.fbw.s1 at the cockpit dash and
equipment.transponder.s1 on the roof. Both are already fitted; their exact
column-major model-frame transforms are checked in as a regression fixture.
No native parts are removed. This overlay is provisional static placement, not
canonical hull installation or mechanical/thermal qualification. A changed hull
hash refuses it; explicit replacement lists are held until a complete moving
geometry/collision consumer exists. Authored hull sockets need no overlay.

The2026-10-10 checkpoint merged installed registry records into main. A normal
`cargo build -p universe --bin freefall` now includes them. To inspect a later
fso-only record set without merging it:

```sh
UNIVERSE_REGISTRY_ROOT=/home/alexm/git/universe-fso/standards \
 /home/alexm/git/planet-trees/.venv/bin/python \
 /home/alexm/git/planet-trees/tools/scenery_job.py --memory-gib 8 --threads 2 -- \
 cargo build -p universe --bin freefall
```

`UNIVERSE_REGISTRY_ROOT` is a build-time setting shared by schema generation and
registry embedding. The normal repository registry remains the default; no YAML
or generated content is copied. Main's explicitly authored game prices inherit
registry renames, so the MC-07 hammer keeps the old key's price when previewing
fso. Explicit prices under new keys win; ambiguous inherited prices are refused.

Run from the universe repository root:

```sh
UNIVERSE_SCENARIO=showship UNIVERSE_HULL=assets/models/mc07.glb \
 UNIVERSE_STUDIO=1 UNIVERSE_PAUSED=1 UNIVERSE_SETTLERS=0 \
 UNIVERSE_ENGINE_THREAD=0 UNIVERSE_DIST=100 \
 UNIVERSE_EQUIPMENT_MOUNTS=/home/alexm/git/blender/mc07-static-fit/mount-overlay.json \
 /home/alexm/git/planet-trees/.venv/bin/python \
 /home/alexm/git/planet-trees/tools/scenery_job.py --memory-gib 8 --threads 2 -- \
 target/debug/freefall
```

For close inspection add `UNIVERSE_INSPECT_SLOT=transponder` (or `computer`) and
set `UNIVERSE_DIST` to a few metres. This changes the observer target, not ship
physics or asset placement. The observer HOME SHIP action restores the central
target. `freefall-studio`/`--studio` remains the separate interior layout tool.

Imported hulls retain registry slot names and sizes for kinds the fitter supports,
instead of dropping additional air/water/cabin slots in a standard inferred list.
Items absent from the explicitly priced playable catalogue remain held and are
logged, not assigned invented prices or functions. Base required systems still
must exist. Unsupported gear/access/thermal/engine slot kinds remain outside
that fitter, and no installed package changes this by itself.

Independent four-variant gear deployment evaluation now passes all129 poses;
see `ships/mc07-gear-motion-review.md`. This remains a diagnostic gate.

Remaining integration: moving gear, ramp, clamps and hammer need combined-hull
interfaces plus their full motion/collision/interaction consumers; radiators
need accepted placement. CH-S2 is not the MC-07's fitted torch drive and must
not replace it. Those remain native/held; static availability is not mechanism
acceptance. Native ramp rendering and collision paths are unchanged.

Validation: full workspace against fso passed (129 tests, 0 failures), including
mount composition, hull fixture SHA, registry slots, price aliases and paused
pilot scheduling. Repository registry generation exited clean. Vulkan on the
RTX 5090 draws both installed packages; full-ship and close captures/logs live
under `out/review/mc07-mounted-v1/`. Close inspection permits entering the hull
bounding sphere and uses a 2 cm camera near plane. These controls change only
the development view. The first full-ship screenshot used the same placement
consumer before this close-camera adjustment.

Right-handed variant review (Blender `20261010T200919-blender-7bce`): bake model-X
reflection into vertices, reverse triangle winding, transform normals and tangent
handedness, and conjugate node/bind frames `S M S`. Proper hull mount frames then
remain determinant +1. With the same scalar angle convention, rotation axes are
axial vectors (`det(S) S axis`), while anchor positions/directions transform as
polar vectors. Preserve control ordering, units, limits and neutral state.

Consumer fixtures must include exact node names and parents, neutral GLB binds,
parent-local anchor frames and axis conventions, primary mount, all-node absolute
reference transforms across the original sampled control domain, and mirrored
world-vertex equivalence at neutral and those poses. Include asymmetric poses
and extrema; report maximum vertex/transform error, winding/normal checks and
no-scale/determinant checks. Retain fixed support/contact/tool interfaces for
ships' combined sweep. Registry owns variant keys/fit identity. This accepts the
representation and fixture contract, not moving-device enablement or installation.

The right-handed hammer also has an explicit 14,000-credit game price, matching
fso `a8fca58b` and registry reply `20261010T201758-registry-308c`. It is not a
rename/inherited alias. This preserves its inert fitted mass/box in the fso
preview without inventing pricing or enabling breaker mechanics. The2026-10-10 checkpoint includes the right-hand record and explicit price on
main. The approved mining-hammer rename is also present.
