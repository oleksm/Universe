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

## Follow-up: rim–patch seam gate and source binding (planets 0281)

Planets' replay found staged rim–patch gaps up to 2.298 mm. All came from startup groups built over one coarse
patch (level 1–10) spanning the whole footprint, whose shared vertex lies kilometres from both f32 origins; every
settled group stayed at or below 4.3 µm. The worker now refuses a group whose rim meets a patch beyond 10 µm
(`rim meets a patch beyond the seam budget (coarse support)`, with the vertex, gap and distances), so those frames
are not ready instead of published. The trace's source hash is now the SHA256 of the bytes this process read for the
very `Surface` the world holds (`pgs_preview::loaded_hash`), checked each frame; the environment variable is gone.
Crest 1 rerun (`crest1-walk2`): every frame bound; two coarse refusals (8.0 mm at a level-1 patch 233 km out,
75 µm at level 7); first ready frame 148, then no frame not ready; staged rim–patch max 4.3 µm.

## Follow-up: a real walker, moving appearance, rebuild debounce

- **Walker.** Dev scenario `groundwalk` (lowflight's place, set down, stepped out, walked clear) and
  `UNIVERSE_PGS1_WALKER` (after 20 s, forward held through the game's own walking commands, as W is). The trace
  marks each frame `walker` while the crew is outside; the root says `walker` only if every frame did. Crest 1
  (`walker-crest1`, trace `185cc8492b64`): 1,618 frames, every one the walker's; 102.7 m walked; 16 groups; ready
  from frame 141 with no gap after; dry contact ≤ 2.0 µm; peak extra GPU 12.1 MB. Checker failures: frames ≤ 140
  only (startup, no group).
- **Rebuild debounce.** The ground under a group changing must hold 30 frames before a new group is built for it.
  From 40 m up at 2.5 m/s the old rule rebuilt every 4–6 frames (66 jobs in 720 frames, peak 15.1 MB, 122 frames
  not ready while frame capture slowed the run). Now (`high-walk`, trace `ee047d5f1196`): 241.6 m in 2,250 frames,
  30 groups, no gap after frame 133, dry contact ≤ 1.6 µm, peak 11.0 MB.
- **Moving appearance.** 294 consecutive frames from 40 m, 60° down, camera walking 2.5 m/s, spanning about seven
  group swaps (`burst`, pre-debounce build). Terrain pixels changed at most 10 grey levels frame to frame at the
  99.9th percentile; the only larger changes were HUD (a message fading in, a distance marker). No pop at a swap
  and no visible footprint edge. Neutral shading: a weak appearance test; it does not show normals, colour or
  texture differences across band, rim and patches. Captures deleted; statistics above.

- **Frame time (visible window).** With the owner's leave, a visible window (1440×810, vsync, 120 Hz display),
  the real walker from crest 1 (`visible-crest1-long2`): 16,762 consecutive traced frames, 139.9 s, 193.6 m walked,
  51 groups. Frame interval from the trace's `wall_s` (wall clock per frame): p50 8.33 ms, p95 8.93, p99 9.30,
  p99.9 11.84, max 21.70; none over 33.3 ms, one over 16.7. Ready from frame 298, never not ready after; peak
  extra GPU 8.1 MB. The planets checker (`check.json`) fails frames 2–297 only (startup, no group).
  The run before (`visible-crest1-long`) had 68 frames of 33–83 ms, every one on a 120th frame and growing with the
  run: the trace rewriting its whole file every 120 frames. The trace now appends each 120 frames to
  `<trace>.frames.jsonl` and writes `trace.json` whole only at exit.

## Not established

- **Appearance.** Screenshots show no visible hole or seam at the views taken (neutral shading, HUD on); the rim's
  normals are its own and band/rim/patch shading across the square's edge needs a close look.
- **The journey.** The walker stands at the place directly (set down there); orbit-to-walk and reboarding are not
  in these runs.
- **GPU depth.** Submitted heights are CPU ray casts of the submitted geometry, not depth readback.
