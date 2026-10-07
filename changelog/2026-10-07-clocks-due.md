# One way to run engine work at a period (audit phase 4, #13)

- `clocks::due(tick, s)`: engine work inside the realtime step runs on the ticks a whole multiple
  of its period. The dead-man check, traffic control's look, reading the ground ahead and the
  recorder's slices use it, in place of three spellings. World clocks (economy, boards, levy)
  keep catching up in game time at their records' periods.
- `docs/tick-tree.md` §8 says which periods are which: world clocks (records), the engine's own
  bookkeeping rates (no records), and clients' (pilots' thinking, news, the newsroom), which stay
  the client's.
