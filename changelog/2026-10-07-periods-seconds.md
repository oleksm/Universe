# Periods in seconds, not ticks (audit phase 4, #13)

- Four periods were tick counts written for a 60 Hz tick; at the clock's 200 Hz they ran three and
  a third times as often (and the delay was three and a third times shorter): traffic control's
  look ("ten times a second": 33), reading the ground ahead ("twice a second": 6.7), a posting's
  command delay (k: 33 ms became 10 ms) and how late a posting may be (0.5 s became 0.15). They're
  seconds now (0.1 s, 0.5 s, 2/60 s, 0.5 s), in ticks by the realtime clock (`clocks::ticks`), as
  they were meant.
- `TICK_BUDGET` stays a count of ticks: it's what a frame may spend, not a time.
