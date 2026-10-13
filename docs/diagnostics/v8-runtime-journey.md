# v8 shared runtime journey

Bounded task `20261013T102453-engine-0cd8`, tested on main `0c484a56`, 2026-10-13. No installation, main merge, physics, guidance or registry changes. [Measured result and provenance](v8-runtime-journey.json).

The headless command test completes **orbital initial state → 30 s coast → continuous descent → lowflight → dry landing → hatch exit → walk → jump → return to ramp** through the actual shared runtime. **Reboarding fails.** Return to orbit, eviction/revisit, input devices and a continuous rendered GUI journey remain unverified. This is a partial runtime result, not acceptance of the whole planet experience.

| Operation | Evidence |
|---|---|
| Source readiness | Hash-verified 167,175,056-byte v8 `surface.pgs`; `Surface::read` completes before constructing `World::with_surfaces` and before the first ship tick. |
| Orbital initial condition | Registry host radius 6,281,370 m; initial radius 6,381,370 m; tangential circular speed 7,705.783812 m/s relative to the moving body; actual shared gravity integration for 30 s without thrust. This does not prove a complete stable orbital revolution. |
| Descent and landing | Existing `landing::autopilot` commands, then `World::step_ship`, no flight state resets after initial spawn. Landed after 2,330.14 simulated seconds / 80,583 flight ticks, intact, approximately 0.394 m lateral error before contact. |
| Exit | Existing `Person::step` interact action emits `SteppedOutside`. Drover uses its built-in hatch transition; this is not a walk down an authored ramp or through a modelled interior. |
| Walking | 1,200 shared walker ticks at 60 Hz while the landed ship and world also advance; maximum excursion 12.819640 m; all 1,200 canonical samples dry. Ground clearance outside the jump window is consistent with zero to floating-point roundoff. This measures the canonical contact sampler, not render agreement or physical accuracy. |
| Jump | One normal jump command; maximum sampled canonical ground clearance 0.444574 m. |
| Reboarding | Returned to `Some(Ramp)`; normal interact emits no boarding event and leaves the person outside. `crew::inside_hatch` returns `None`. |
| Resources | 804.147169 kg fuel consumed; 29,195.852831 kg remaining; hull fraction 1.0. Final headless run 7.874 s wall time, including source/world startup. This is not a frame-time benchmark. |

The immutable source is SHA256 `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`, from `planet-trees/out/planet/landscape_v8_seed83_20/landscape_002/review/reader`. The lab retains the registered host's mass, gravity, rotation and atmosphere. The source radius is 6,371,000 m: unit directions and absolute heights are preserved, horizontal lengths and slopes follow the explicitly different host radius. Time starts at 6.3 hours for the daytime lab. The target is a test `PadFrame` at the supplied corridor direction, using its canonical terrain height plus the ship's rest height; no port, terrain flattening or ATC record is inserted. This harness directly invokes the existing guidance helper; it does not validate normal cockpit target selection or clearance workflow.

## Reboarding reproduction against the live call

The fresh procedural Drover has `shape.walk == None`. The harness omits a layout initially, but the failure was independently reproduced by transferring the **same existing World, ship and crew state** into `Universe::from_world`, calling the normal `set_layout` with a fresh empty Drover deck plan, and issuing the normal `Universe::walk(interact)` action. The result remains outside, with no boarding event.

`crates/core/sim/src/universe.rs::set_layout` returns before constructing an interior when the shape has no walk mesh. `Universe::walk` therefore supplies no layout for this fresh procedural ship. Independently, `crew::inside_hatch` rays only the ship's colliders and does not accept the layout argument; its floor lookup returns `None`. The Ramp interaction consequently has no valid interior destination. No fabricated floor, forced seat assignment or crew teleport was added by the harness. This result is specific to the fresh stock Drover; native hulls and ships with different entry geometry were not tested.

## Preserve the failed first attempt

The first circular-orbit attempt engaged the same landing guidance immediately and **crashed after 711.02 simulated seconds while still in Approach**, with 29,750.61 kg fuel remaining. Its full input/state/event trace is retained. A separate 1,000 m lowflight start successfully landed in 72.78 s, exited, walked and jumped but reproduced the same reboarding failure.

The successful orbit attempt changes the pilot's timing by coasting for 30 s before engaging the unchanged guidance. It does not repair the first failure or establish robust landing guidance for arbitrary starting conditions. An additional run reproduced the successful flight state and outcome. Earlier diagnostic target-distance fields mixed post-step ship state with pre-step body position; the final trace records all `*_before` metrics at the same pre-step time. This telemetry correction does not change the flight commands or trajectory.

## Artifacts and reproduction

`tools/review/v8_runtime_journey.rs` is a standalone diagnostic linked to existing tested main rlibs; `tools/review/v8_runtime_journey.py` records exact library, harness, executable and handoff hashes. It does not modify a shared Cargo target. Full per-tick commands, poses, time steps, fuel, state and events are retained at:

- `/home/alexm/git/universe-ships-pgs-journey/out/review/v8-runtime-journey/orbit-final/trace.jsonl`
- `.../orbit-final/result.json`, `provenance.json`, and `foot-samples.json` (103 selected foot/jump samples, body-frame metres, live time and canonical contact values).
- `.../baseline/trace.jsonl` and `result.json` (failed immediate-guidance attempt).
- `.../lowflight-live/trace.jsonl` and `result.json` (separate downstream control).

Run from the isolated worktree, substituting an existing compatible tested main dependency directory if needed:

```sh
python3 /home/alexm/git/planet-trees/tools/scenery_job.py --memory-gib 8 --threads 2 -- \
  timeout 600 python3 tools/review/v8_runtime_journey.py \
  --deps /home/alexm/git/universe-ships-ready-main/target/release/deps \
  --handoff /home/alexm/git/planet-trees/out/benchmarks/planet-scale-integration-v1/handoff.json \
  --out out/review/v8-runtime-journey/repeat --mode orbit
```

Use `--mode lowflight` for the explicitly separate downstream start. The launcher wraps compilation and execution in the shared 2 CPU / 8 GiB / 600 s guard. Raw outputs remain in ignored review directories; the compact measured report and diagnostic sources are committed. These linked artifacts are main's existing shared simulation, not an alternate physics or walking implementation.

Engine owns the PGS reader and actual rendered-contact audit in `engine-pgs-walk/docs/diagnostics/v8-orbit-walk-runtime.md` (`232710f6` code, `ac27d5a5` report). Its submitted-mesh and frame-time evidence must remain separate from this headless result. The reported render-contact control improved, while the 1280×720 p95 frame gate remains failed at 51.59 ms with a surface-acquire/compositor confound. Neither that result nor this command trace certifies production readiness.
