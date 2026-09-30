# 2026-09-30 — Architecture: physics kernel / world / avionics / orchestration

## Decision

The user asked for the physics engine to be completely independent of the navigation programs,
and "ruthlessly stable and fair regardless of what we do on top of it" — because stations,
spaceports, gates and ships will later be things players *construct*. Adopted a layered
architecture (reference: `docs/architecture.md`):

- **physics kernel** (`universe-physics`): rail bodies, rigid bodies, gravity, integration,
  colliders (surface, polytope with cut-outs, ring + trigger), facts (contacts, triggers),
  audited ops (weld, relocate), queries (incl. simulate-a-copy). No game concepts.
- **world** (`universe-world`): content (galaxy, systems, terrain), structures, ships as rigid
  body + devices, device and contact rules (docking port, landing gear, gate device, hyperdrive
  device, damage), traffic control service, events.
- **avionics** (`universe-avionics`): ship software — nav computer, guidance, planner,
  autopilots (dock/land/gate/hyper/route); sensors in, `ShipCommands` out; future modules.
- **sim** (`universe-sim`): orchestration of ticks, ships and traffic; save/load.

Why: the review found physics and navigation interleaved in one 2,600-line `universe.rs` —
autopilots running inside the integrator's substep loop, the hyperdrive half engine / half
navigation computer, physics decisions reading clearance state, physics clearing navigation
state, navigation fields on the ship's physical state, and the planner carrying its own second
integrator (the source of several past bugs).

## Plan and process

`docs/refactor-plan.md`: four implementation phases (kernel → world → avionics → orchestration &
game), each leaving everything compiling with all tests passing, then a parallel verification
phase (architecture audit, regression vs baseline, game smoke test, code review) and a fix pass.
Executed by a multi-agent workflow. No git in this repo, so each phase backs up the tree first.

Baseline recorded in the plan (29 tests + 10 long runs green; traffic 0 crashes; bench numbers).
Pre-refactor fix: the long landing ETA-smoothness test assumed a flight plan every frame; with
the autopilot's hyperjump there is none during hyperdrive, so the test now measures the
countdown after the jump.

## Phase log
