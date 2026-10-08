# The game's App in groups (audit phase 5)

- `App`'s 99 fields: 30 of them grouped. `app.panels` (`Panels`: navigation map, economy, galaxy
  map, market, shipyard, standards, planet studio, preview), `app.plan` (`PlanState`: `current`,
  `prev`, `blend`, `age`, `serial`, `cost`, `every`, `follow`, the guide, liveliness, the ETA shown),
  `app.net` (`NetState`: the link's `status` and when read (`at`), the `nodes` in sight, `news`, the
  `newsroom`) and `app.caches` (`Caches`: ground patches, globes, worlds' maps, rocks, rigs,
  `sky`). Done at a quiet point agreed with the ships session (their `app.shipyard` is
  `app.panels.shipyard` now). interior.rs untouched (its split is the ships session's).
- Flight, the deck, the navigation map and a market look as before; 98 tests pass.
