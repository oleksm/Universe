# Guidance for the follow programs

Keep at range (N), orbit (U) and closing on a rock (3, mining) now guide like landing and
docking do:

- **A phase banner** across the top: what's followed, the steps — CLOSE IN > MATCH DRIFT >
  ANCHOR (Y); JOIN ORBIT > ORBIT; CLOSE TO RANGE > HOLD — the current one boxed, the distance
  to go and the ETA.
- **In the view**: a dashed path from the ship to where the program is taking it, the goal
  marked (green once there); the orbit's circle; the range shell kept at; on a rock, the
  anchor's reach ringed on the surface under the ship (green when in reach and drifting slowly
  enough).
- **The numbers**: distance to go and closing speed; on a rock, the gap to the surface and the
  drift against it, against the anchor's limits (within 30 m, under 0.5 m/s) — Y TO ANCHOR
  when both are met. X lets go of any of them.
- Dev scenarios `closing10`, `closing`, `orbitrock`.
