# Dev scenarios as a table (audit phase 5)

- `dev.rs`'s 1,700-line `match` is a table, `SCENARIOS`: each entry the names it answers to, a
  line on what it shows, and its function (one per scenario; variants share one and read the
  name they were asked by). What every scenario starts from (home system, its bodies now, the
  station, the planet, the outermost reach) is `Ctx`, and `observe` a plain function.
- The printed list was a hand-kept string of 60 names (the match knew 120 more); an unknown name
  now prints the table itself, names and what each shows, so it can't go stale. `look_<hull>`
  keeps its prefix.
- Same behaviour: the scenarios the frame checks use (docked, low flight, `look_drover`) look as
  before.
