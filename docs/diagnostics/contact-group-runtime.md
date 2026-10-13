# Contact group at runtime: publication, pinning, re-anchoring, GPU runs

Task `20261013T151045-planets-5f73` (engine-claude), branch `engine-claude-walk`. Opt-in only
(`UNIVERSE_PGS1_CONTACT_GROUP`); default renderer, physics, source and registry unchanged. CPU group build:
[contact-group-excision.md](contact-group-excision.md).

## How it runs (`crates/game/src/terrain_lod/group.rs`)

- **Two selections a frame.** The natural one (as the eye would have it) drives patch making and the contact
  snapshot. The drawn one keeps the **active group's replaced patches pinned**: never split, their holders always
  split down to them. A group therefore stands until the eye nears its core's edge; LOD churn under it no longer
  drops it.
- **Build.** The shared worker (`universe_terrain_contact::worker::group`) runs on the ground pool from the natural
  selection, only when it is settled within 300 m of the eye. Wanted when there is none, the eye is within 8 m of
  the core's edge (24 m from the centre), or the natural selection under the group changed.
- **Reserve and stage.** All of a group's meshes (band, rim, replaced patches) are reserved at once in a 16 MiB
  `MeshBudget` that counts active, uploading and retired-but-not-GPU-released together, then staged with
  `Frame::prepare_mesh`.
- **Publish atomically.** At the next selection, a staged group whose meshes are all GPU-resident and whose
  replaced patches are still resident as built becomes active, replacing the old one in the same frame; then the
  drawn selection pins its patches. Drawing substitutes every replaced patch at once and adds band and rim.
- **Retire.** A group goes when a patch it replaces is no longer drawn as built (or the source changes). Its
  meshes are released to the renderer at once (`Frame::release_mesh`, new, re-sent for 600 frames since a frame
  can be replaced before it is drawn); each reservation still waits for submitted GPU work. Before this, retired
  groups stayed for 600 *renderer* frames, minutes on a throttled test workspace.
- **Dependency rule.** A group depends on the patches it replaces (they cover its footprint, and the selection never
  overlaps itself). Patches beyond them changing meet them as any neighbour does, skirts kept outside the square.
- **Trace.** `UNIVERSE_PGS1_READINESS_TRACE=<path>` writes `planet-readiness-trace/1` (subject `camera`): per frame
  the canonical bed height and water, the submitted ground height ray-cast from the pieces actually submitted that
  frame (band, rim, replaced and resident patches as the shader morphs them), the group, and GPU bytes; plus
  `<path>.events.jsonl` (jobs, refusals, staging, publication, retirement with witnesses). The source hash is the
  one the run names (`UNIVERSE_PGS1_SOURCE_SHA256`); the trace carries it, it does not prove it.
- **Camera walk (dev).** `UNIVERSE_PGS1_CAMERA_WALK=<m/s>` (after `UNIVERSE_PGS1_CAMERA_WALK_AFTER`, 20 s), paused:
  the camera alone walks along its heading 1.7 m over the canonical ground. Nothing physical walks.

## GPU runs

`lowflight` at each site, `UNIVERSE_ALT=1.7`, paused, day (6.3 h), frozen v8 source `4b30e7ed…`, window on
workspace 8, 2,400 frames; camera walking 1.4 m/s from 20 s, except the shore. Script, traces, events, audits and
screenshots: `out/review/contact-group/gpu/` (local). Checked with planets' `tools/planet_runtime_trace_check.py`.

| Run | Frames | First ready | Ready | Not ready after first | Groups | Active retired | Travel m | Dry contact max m | Peak extra GPU MB | Checker failures | Trace |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| crest 1 walk | 2180 | 12 | 2170 | 0 | 17 | 0 | 65.7 | 1.5e-6 | 8.2 | 10: startup | cc55bc5eeb4d |
| crest 2 walk | 2156 | 9 | 2149 | 0 | 19 | 0 | 56.9 | 1.7e-6 | 8.3 | 7: startup | e5aa26e69c4f |
| face join walk | 2168 | 11 | 2159 | 0 | 24 | 0 | 68.6 | 1.7e-6 | 7.4 | 9: startup | d318d51b4659 |
| shore (still) | 2198 | 8 | 2192 | 0 | 9 | 0 | 22.5 | 9.2e-7 | 7.7 | 6: startup; 882 wet | 7297a1bc1b8d |

Every failing frame is either before the roots and first group exist (not ready, no group: reported as such) or,
at the shore, over water (`not_known_dry_ground`, correctly not a walking pass). The shore camera moved 22.5 m
once, at 0.01 s simulated, when the world settled the ship the scenario had put 1.7 m over the water; the trace
records it. Re-anchors: each new group replaced the active one in the same frame.

## Not established

- **Frame time.** The test window on workspace 8 is throttled by the compositor: the renderer drew about one frame
  in eight game frames. No frame-time claim; the 33.3 ms p95 gate needs a visible window.
- **Appearance.** Screenshots show no visible hole or seam at the views taken (neutral shading, HUD on); the rim's
  normals are its own and band/rim/patch shading across the square's edge needs a close look.
- **A walker.** The subject is the camera; actual walker inputs and the orbit-to-walk journey are not in these runs.
- **GPU depth.** Submitted heights are CPU ray casts of the submitted geometry, not depth readback.
