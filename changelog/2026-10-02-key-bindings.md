# Key bindings by name, in a tree of scopes

- **Every action's key is a letter of its name**: the first one free in its scope, else the
  next along the name (`keys`). Scopes form a tree: EVERYWHERE; FLYING (any mode), and under
  it NAV, COMBAT, MINING; the NAVIGATION MAP; the OBSERVER. A letter is used once along a
  branch; sibling modes may share one (P is proximity in NAV, the pulse laser in COMBAT,
  prospect in MINING). Each scope keeps its moving keys (W A S D Q E flying, W S browsing).
- **The letter is lit in the name** on every cell (and underlined), as well as shown.
- The bindings, as they stand:
  - EVERYWHERE: M MAP, R MARKET, C CARGO, O ECONOMY, B COMBAT, I MINING, V VIEW, F FOOT;
    F1 help, TAB watch, BKSP respawn, F2 labels, F4 grid, F6 pause, F3 profiler, F5/F9
    save/load, F8 mute, F12 screenshot, , . time warp.
  - FLYING: L LOCK, H HYPERDRIVE, U AUTOPILOT, K KEEP, T ORBIT, G LET GO; W S throttle, A D Q E
    roll and yaw (shift: thrusters), arrows, mouse.
  - NAV: N CLEARANCE, P PROXIMITY (collision warning). COMBAT: SPACE gun, P PULSE LASER.
    MINING: P PROSPECT, Z ZERO IN, N ANCHOR, X EXCAVATE.
  - MAP: T TARGET, L CLEAR TARGET, A ADD STOP, D DROP STOP, E EMPTY ROUTE, U SETTLER ROUTE,
    G GALAXY; arrows select and change system.
  - OBSERVER: N NEXT STAR, P PREVIOUS STAR, H HOME SHIP, T TRACK SETTLER; wheel, [ ].
- Z and X no longer set the throttle full or cut (hold W or S).
- **F1 shows the tree**, generated from the table (so it's always what the keys are).
- **The maps' actions are a grid** like the flight's (navigation map, galaxy map).
- Mode-scoped actions only work in their mode.
