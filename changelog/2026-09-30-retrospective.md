# 2026-09-30 — Retrospective: what made the pirates and collisions work slow

User: "what took us so long to implement? Too many tests, slow tests, too many edits, brittle code
due to accumulated tech debt, or what else?"

Estimated shares of the time on the pirates and collisions features (not measured, from the
session):

1. **Waiting on whole-world simulations (~35–40%).** The unit tests are fast (under 5 s) and
   incremental builds take about 2 s. The ignored traffic runs take 1–25 minutes, and were rerun
   after nearly every fix (about 7× for pirates, 5× for collisions). Some ran at 20× warp, where a
   craft gets one command per 0.33 s frame, which exaggerated problems the game (1×) doesn't have.
2. **Guessing before instrumenting (~25%).** Fixes tried against one aggregate number. The targeted
   probes, each written in minutes, named each cause at once:
   - the crash log with role, hunt and route state;
   - the gate-chase trace;
   - the collision snapshot of both ships the frame before.
3. **Threading one concept through every layer (~20%), partly design debt:**
   - one new field ("a clearance has a pad") touched about 10 files across all five crates;
   - every new event variant breaks the exhaustive matches in `avionics::observe`, `game/main.rs`
     and `game/sound.rs`;
   - `computer::autopilot` now takes 9 parameters;
   - traffic control infers claims by scanning all ships every frame (`keep_pads`), which led to
     two real bugs (spawned ships' pads, pads freed during climb-out);
   - the clearance and route lifecycle is changed in about 6 places;
   - `Universe` keeps growing.
4. **Editing method (~10%):** scripted find-and-replace with assertions fails loudly on stale
   context. That's safe, but it costs rereads.
5. **Visual checks (~5–10%):** screenshots read one by one, and a couple taken of a stale binary.

The number and speed of the tests were not the problem.

Remedies proposed before the next feature:
1. Small scripted interaction tests (2–3 ships, seconds each) for iterating. The big traffic run
   becomes an occasional benchmark, at 1× only.
2. A flight recorder: the last N seconds of state for any ship that crashes or collides, printed
   automatically.
3. Traffic control as one service with explicit claim and release from events, instead of
   per-frame inference.
4. An `AutopilotInput` struct instead of long parameter lists, and default arms (or a message
   trait) for game-side event handling.
