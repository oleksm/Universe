# 2026-09-30 — Star gates between systems

## Summary

A gate network links the home system and its 4 nearest neighbours (5 systems, 1–3 gates each,
fully connected). Gates are rings you fly through; transit is a short, visible trip that keeps
your motion relative to the gate. The hyperdrive stays as is for now.

## Player-facing

- **Gates**: 3 km wireframe rings (orange), inertially fixed, orbiting the station's planet (or
  another planet / the star). Named for where they lead: "Gate to Ussodive".
- **Nav map (M)** lists gates (kind GATE, orange circle on the chart). Lock one, **R** requests
  transit clearance (within 50 km), **K** flies it (approach → line up → run-in at 100 m/s).
  Hyperdrive with a gate targeted drops out 20 km away.
- **Transit guidance**: the ring's axis, the run-in point, drift, and the flight-plan tunnel;
  panel with range, closing speed, `PASS AT 100 M/S, MAX 300`, axis offset, hints; banner
  `1 APPROACH GATE > 2 LINE UP > 3 TRANSIT RUN` with ETA.
- **Transit rules**: through the opening under 300 m/s → transit; touching the ring, or faster
  than 300 m/s → destroyed.
- **The trip**: ~5 real seconds in a tunnel of rings rushing past, "GATE TRANSIT TO X - ARRIVING
  IN 3.0 S", a rising whoosh; then you come out of the paired gate with the same velocity,
  position offset and attitude relative to it ("WELCOME TO THE X SYSTEM").
- **Galaxy view**: zoomed out in observer mode, gate links are drawn between systems, with the
  network's stars circled and named in orange.

## Simulation

- `BodyKind::Gate` (massless, "artificial" like stations: ignored by gravity and hyperdrive
  surface clearance), `Body::link` = destination system; `StarSystem::add_gates`, `gate_to`.
- `Universe::build_gate_network`: spanning tree (each system to its nearest linked system with a
  free slot) plus up to 2 extra short links; max 3 gates per system. `gate_links`, `gate_links_of`.
- New `gate` module: `GateFrame`, `crossing` (plane crossing through the opening vs. ring hit),
  guidance, status, autopilot. `NavTarget::Gate`, `ShipState::Transit`, events `GateEntered`,
  `GateArrived`, `GateTooFast`, `ClearanceGranted { kind: Dock | Land | Transit }`.
- Planner handles gates; a gate plan "arrives" when it passes through the ring.
- Bug found by the tests: the crossing check measured the previous position against the gate's
  *current* position; gates orbit at ~3 km/s, so this caused phantom ring hits. It now uses where
  the gate was at the start of the step.
- Tests: network shape (5 systems, 1–3 gates, connected), autopilot transit keeps velocity
  relative to the gate (entry and exit both (0, −100, 0) m/s), ring hit and too-fast pass are
  fatal. 20 sim tests pass.

## Dev

- Scenarios: `gate`, `gateauto`, `transit`, `gatearrive`, `network`.
