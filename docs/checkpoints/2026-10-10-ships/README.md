# Ships checkpoint — 2026-10-10

This checkpoint preserves the MC-07 fit studies and their documented gates. It does not install the combined hull, enable replacement drawing or accept moving equipment. Engine owns the main merge under the owner's checkpoint authorization.

`studies.tar.gz` contains the ships study directories from `/home/alexm/git/blender`. `manifest.json` lists every included file and SHA256; all archive members were read back and verified. Extract into a separate review directory. Scripts retain their original absolute workspace paths; extraction alone does not make them relocatable. Redundant `.blend1` backups and Python bytecode are omitted. The installed equipment packages and upstream Blender asset sources belong to the assets/Blender checkpoint, not this archive.

The archive includes the combined candidate GLB and both source scenes, composition scripts, sampled sweeps, proper mount matrices and exact native replacement lists, component fit fixtures, renders, static placement overlay, radiator rejection evidence, and the ships right-package release decisions. The combined candidate retains original node names and 42 production marker frames; it is not the production `assets/models/mc07.glb`.

## Dependencies and preserved state

- Registry fit mapping: fso `4bb9df3e`, following right-hand identities `a8fca58b`. `gear_1` is front-right, `gear_3` rear-right, `hardpoint_2` hammer-right.
- Installed right static packages: front `dbcdd64e`, rear `f6ede496`, hammer `2b0385ad`; loader partial results `563513fa`, `d9168141`, `a00e2626`. Exact installed model hashes are in `mc07-combined-fit/installed-right-packages.json`.
- Left gear v1, ramp v1 and hammer v2 remain as recorded in their integration plans. Installed packages reside in `/home/alexm/git/freefall-assets/models`.
- Engine's shared computer/transponder preview is separate engine work. Its overlay is pinned to the original hull hash and must not be silently rebound to the candidate.
- Moving gear/ramp/hammer replacement still requires complete consumers and production hull acceptance. `planned-bindings.json` is explicitly not an enabled static overlay. Fixed support geometry is required; do not suppress native mechanisms prematurely.
- Hammer-right static release covers exact reflected geometry only. At pose24/deployment0.375, chisel and well wall have nominal zero clearance, with measured0.596um numerical plane crossing. The combined motion gate remains held. The two driver-ID metadata fixes reproduce the previous fixture bytes when reversed; all112 pose frames are unchanged.
- Radiator placement remains rejected/pending redesign; RCS/hardware/thermal qualification is not granted by checkpointing.
- Ramp isolated30mm threshold/walking evidence does not replace full combined-hull/runtime boarding validation. Deployed tread chamfers have documented brief support loss.
- Structural/load, full working-volume, continuous-motion and GPU qualification remain limited to what each review explicitly states.

## Worktree and external files

The ships worktree was clean at `33ebfa58` before adding this checkpoint. No game source or production assets were changed in this checkpoint. The ships-authored registry fit change was already committed/pushed as `4bb9df3e`; its worktree was clean when checked.

The original study files remain outside Git under `/home/alexm/git/blender`; their checkpoint copies are fully inventoried here. Live Blender/asset work owned by the Blender agent is not declared clean by this report. No main merge was performed by ships.

Validation: archive member hash verification; no runtime tests needed for this evidence-only commit. Component test results are preserved, not rerun or broadened into new acceptance claims.
