# The shipyard (T3b)

Docked at a station, the **shipyard** (key **Y**, a cell in the top bar). It's the start of the
ship planner.

- **Left:** the ship's slots (name, kind, size) and what's in each.
- **Right:** for the slot picked, every module of its kind that fits it, with mass, power draw,
  price and what it does, plus "EMPTY" where it isn't a base block. The one fitted now is
  starred.
- **Below:** the ship's numbers now and with the module picked:
  - dry mass, tank, hold;
  - power drawn against made;
  - main drive, thrusters and lift as accelerations at full load;
  - the turning envelope;
  - the autopilots it runs.

  Changes stand out. Also what it costs (less the 60% the module taken out fetches), or why it
  won't do: "WON'T DO - NO POWER: EVERY SHIP MUST CARRY ONE".
- **Keys:** ↑/↓ slot, ←/→ module, ENTER fits it. That's a world command (`Command::Refit`),
  checked and paid like any refit.
- The key's letter: newer actions go last in the key table, so the keys already learnt stay put
  (SHIPYARD takes Y; HYPERDRIVE keeps H).
- Dev scenario `shipyard`.
- **Visual check pending:** the display is throttled right now (every build, old ones included,
  runs at 48 ms a frame and captures nothing).
