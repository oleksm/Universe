# Shared Studio diagnostics

Ship Studio uses the same profiler, HTTP observer, recording writer and renderer
measurements as the live game. This applies to standalone `freefall-studio`,
`freefall --studio`, and the Studio inside the game; DESIGN, WALK, BALANCE and
TEST DRIVE all call the instrumented shared view functions.

- F3 cycles the frame/asset HUD, scope profile, and hidden HUD.
- Shift+F3 enables the localhost observer, hitch logging and a bounded recording.
  Press it again to stop. Recordings retain the existing ten-second/1500-frame limits.
- Standalone automation can set `UNIVERSE_OBSERVING=1` and
  `UNIVERSE_OBSERVE_PORT=17879` to enable the observer without starting a recording.
- `UNIVERSE_PROFILE=1` enables timing from startup. `UNIVERSE_DEBUG=2` displays it.
- The shared observer exposes `/status`, `/perf`, `/profile`, `/resources`,
  `/screenshot`, `/record/start`, `/record/stop` and `/sessions`.
  Studio reports its actual view and does not fabricate world/crew data.
- Studio `/graphics` explicitly reports unsupported world PBR switches: its
  equipment previews currently use the shared canvas.

Recordings are stored under `$XDG_DATA_HOME/freefall/sessions` (normally
`~/.local/share/freefall/sessions`). They contain frame images, per-frame timings,
`trace.json` for Perfetto, start/end status and resource counters. Recording
screenshots adds overhead; measure steady FPS with `/perf` before recording.
The network observer remains dev-feature-only, as in the existing game.

`RENDER` in the HUD is render-thread CPU time. GPU pass timings are shown on a
separate line when timestamp queries are supported. Scope times across threads
are cumulative CPU time, not frame wall time. Nested scopes must not be summed.

New named scopes cover Studio input, refit, craft construction, budget calculation,
shared equipment projection, design submission, catalogue, walk, balance and drive.
They distinguish CPU projection/submission costs from GPU pass and screen-wait
measurements; the owner's reported 40 FPS is not yet attributed to one cause.

## Reproduce

`tools/diagnostics/studio-observer-check.py BINARY DESIGN OUTPUT --mode design`
checks real live endpoints, a screenshot and a recording with Studio trace scopes.
Run each view in its own process/port; the tool uses an isolated data directory.

`tools/diagnostics/delay-model-open.c` is a Linux test-only preload interposer.
It delays installed-package `model.glb` opens by
`FREEFALL_TEST_MODEL_DELAY_MS` (maximum 60000). It does not alter native hull
physics reads, registry data, assets, or the production loader. Build it with the
command in its header, then launch a capture with `LD_PRELOAD` pointing to it.
Confirm delay log lines; a run without them does not prove delayed loading.

## Startup audit at a85eab28

The preparation shell opens before `App::new`/`StudioOnly::new`. Content validation,
procedural base-model construction, native hull collision commissioning, and
optional overlay metadata parsing are in that worker. Physics readiness remains
a publication gate; no partial collision state is exposed.

Studio still commissions MC07 before opening a hull-free design: an unnecessary
prerequisite but not a main-thread freeze. Showcase, native hull visual and mounted
package decoding use the shared async loader. Missing/rejected/queued equipment
keeps Studio envelope geometry. Mounted moving equipment is intentionally held
pending motion/collision acceptance, not merely pending file I/O.

The first world update still synchronously built globe data and first terrain
patches in the audited revision; engine owns their replacement. Other unmeasured
CPU candidates include procedural ground-block convex hulls and wall/hatch cache
rebuilds on first draw. `send_layout` clones and sends a command; it does not wait
for an asset. No blanket stutter-free claim is made.

GPU baseline evidence under `out/review/loading-audit`: while both CH-S1/S4 model
opens were delayed ten seconds, `pending.png` shows two envelopes, two running
requests and 60 FPS at the configured 60 FPS ceiling. First shell submission was
221 ms, physics-ready publication 796 ms (warm OS cache, not cold-machine timing).
A separate five-second-delay run `ready.png` shows both installed meshes, two loads
and zero failures. These tests establish fallback and eventual promotion, not
nozzle-axis correctness or flight acceptance.
