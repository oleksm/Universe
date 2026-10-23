# Engines as products: real instances, strictly bound nozzles, roles by direction

Route `docs/ships/explorer-propulsion-route.md` steps 1-6 (registry `20261022T192515-registry-db27`).

- `Does::Engine` (thrust, exhaust, efficiency, burns, throttle, gimbal) and `SlotKind::Engine` from the registry's
  `engine` slots: one instance a slot, its own mass. Chemical engines only: the other cycles take their energy
  from elsewhere (reactor, tank pressure, fan power) and stay not made rather than pretend.
- The importer binds nozzles to engine slots by the hull record's `thrusters` exactly (`bind_engines`): a nozzle
  the model lacks, one bound twice, a share outside (0, 1], an engine's shares over 1, an engine slot driving
  nothing: refused. Bindings to other slots stay with the name rule (MC-07 unchanged).
- An engine's role from its push: along the nose (within 30°) main drive, upward a lift, otherwise refused. An
  engine stands for the Drive base block; a ship of engines with none pushing forward is refused.
- `ClassSpec::limits`: per engine, its throttle floor (or on/off) not enforced, its gimbal not simulated, thrust at
  every ambient pressure, ignition and restarts not modelled: stated for acceptance to hold on.
- Engines are carried at price 0 when unpriced (never sold), like command stations and legs. One whose jet carries
  more energy than its fuel gives at its efficiency is held out of the catalogue with a warning, not loaded and not
  failing the content. That holds out every CH record today (S1, S2 hydrolox 9.68 MJ/kg against 0.74 × 13 MJ/kg;
  S3, S4 methalox 5.41 against 0.54 × 10): the registry's to mend.

MC-07: thrusters, fit and masses identical before and after. Tests: `import_binds_engine_nozzles_strictly`,
`import_engines_drive_as_they_point`; the workspace's library tests pass. Not yet: cold-gas RCS beside hydrolox
(registry 6ce7), so an all-engine hydrolox ship can't yet meet the RCS base block with one fuel.
