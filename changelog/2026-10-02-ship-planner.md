# The ship planner (loadout)

The shipyard key opens the **ship planner**, anywhere: in flight it's the planner, and docked at
a station it's the shipyard.

- **A plan:** a hull and a module in each of its slots, from the whole catalogue. It starts as
  the ship you fly.
- **PLAN page:**
  - ↑/↓ picks a slot, ←/→ a module, ENTER puts it in the plan. Slots that differ from your ship
    are marked `*`.
  - Your ship and the plan side by side: hull, dry mass, tank and hold, power, the drive and
    thrusters as accelerations, lift in g (empty / full hold), the turn, the balance (lift and
    drive kept, empty / full), how long a tank lasts at full drive and hovering at 1 g, the
    autopilots, the list price.
  - The plan drawn from above and the side: each module's mount (the slot picked bright), its
    thrusters, its centre of mass at the usual load.
  - In flight, list prices. Docked, this station's: what it carries, at what markup, with stock
    to build it.
- **HULLS page:** every hull with its numbers and price, and the one picked drawn. ENTER starts a
  plan from it, as sold.
- **PLANS page:** the first row keeps the plan as it is. ENTER loads a kept plan, DELETE drops
  one. Plans are kept in the save by content key, so they survive content changes as far as
  their parts do.
- **Building a plan (docked, SHIFT+ENTER twice):**
  - The hull is bought if it's another (your ship traded in), then each slot that differs is
    refitted, through the same commands as before.
  - Before you build, it shows the cost here (the hull less your trade-in, each new module less
    what the old one fetches) and anything this station doesn't carry or can't build.
- The thrusters panel's ship views are shared with it (`thrusterpanel::Picture`, `view`).
- Dev scenarios `planner` (in flight) and `shipyardbuild` (a courier plan, built).
