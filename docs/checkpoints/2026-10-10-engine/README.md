# Universe main checkpoint

Owner requested all universe changes merged to main on2026-10-10.

Integrated inputs:

- Engine main through ca1c55b0, plus local-view audit b80522be and calibration
  305026c6. Includes mounted computer/transponder preview, inert unimplemented
  fittings, PGS terrain/water/category/material/debug readers and diagnostics,
  motion/handed-asset reviews, drainage/boarding/control-authority evidence.
- Registry fso d7220b04, including registry-wip at the same checkpoint. Merge
  8c586f0f resolves the only conflict to the approved mining-hammer.mc07 price
  rename, retaining the explicit mining-hammer.mc07-right price.
- Ships f40a6deb, merged8950a666: equipment source notes and a verified264-file
  study archive under ../2026-10-10-ships/.
- Planet lab checkpoint b099a4e (source b50380e) on separate repository branch
  checkpoint/scenery-20261010. Its master is untouched. See that repository's
  docs/requests/2026-10-10-planets-checkpoint.md for datasets/dependencies.

All current universe branch tips above are ancestors of main. Blender reports
no separate universe source changes. Installed right packages and loader-partial
records are included through registry; the shared asset payload store remains
external at /home/alexm/git/freefall-assets. No production hull swap or moving
mechanism enablement is granted by this checkpoint. In particular, hammer
chisel/well tangency, radiator placement, combined ramp boarding and remaining
consumer gates stay as documented. The mountain-scenery goal remains unmet.

The six final local-view PNGs/logs are archived in local-views.tar.gz, verified
against local-views-manifest.json. Full precision camera settings, hash identities,
visibility screens and independent attribution arithmetic are in
../../diagnostics/scenery-local-views-v1/. These are development evidence,
not a production world installation.

External data are not silently imported into universe: planet-trees ignored
out/ products and Earth datasets remain local; planet-sim f533968 has two dirty
reference tools outside the planet checkpoint; live Blender source/blockouts
remain owned by Blender. Ships' archived studies are a pinned snapshot.

Validation: registry build passes;132 workspace tests pass, including the
installed-model loader test with the local asset store present; release game
build succeeds. No new balance figures, terrain fit or runtime mechanism
acceptance is introduced by this merge.

Release Vulkan smoke passed120 frames and the computer close-up was inspected.
The log confirms computer/transponder mounted drawing; other missing mounts and
air/water catalogue holds remain explicit. Evidence: mounted-runtime.png/log
and validation.json.
