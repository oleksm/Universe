# Halden Camp: the first mine on a baked deposit

- **A settlement the seed has no port for** (a camp at a mine) gets one where its record puts it
  (`StarSystem::add_spaceports`): Halden Camp on Harvest at 54.81 N 164.44 E; its land and works
  from the registry as any port's (Halden Mine, its power station, its yard on the exchange).
- **A mine digs its claimed deposit** (`facility.claim`): its deposit's ore from the body's
  survey (`economy::deposit_ore`: TRD1-PCU-007264A-01, 1,230 Mt of porphyry copper ore), drawn
  down by what it digs (a recipe's `from_ground`: the made thing drawn from the ground where it
  stands), and when it's worked out, it stops ("DEPOSIT WORKED OUT"). Its pit is set to the ore of
  its deposit's kind.
- In three days it makes 1,500 t of copper concentrate and 20 t of molybdenum, then stops with its
  store full of 108,000 t of tailings and 91,000 t of waste rock: it has nothing to take them
  (asked of the Scientist). Its people eat what's brought from Port Eikir.
- `worlds::World` (a world's survey and energy packages) is `worlds::Survey` now.
- Not yet: an unlicensed claim recorded as an offence (none exists: Halden's is licensed).
