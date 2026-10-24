# The Explorer ground stage loads in the game

- `content`: an uncommissioned hull (a stage) is validated as a body (dry mass, radius, drag area, strength), not
  as a flyer (turn rates, drive, thrusters it hasn't got). Test: the bare-record stage validates (import tests).
- `pbr::bounded_import`: the expanded-payload bound is 256 MiB (the Explorer's 1,040 meshes estimate 130 MiB).
- Registry record 308238a07 and the ships' model and audit (2d2abef64, fe09a168f) merged. Live run and the runtime
  spec: `docs/ships/explorer-ground-install-live.md`.
