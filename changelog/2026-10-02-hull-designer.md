# The hull designer

The ship planner has a **DESIGN** page (TAB to it): a hull of your own, drawn up from a few
numbers.

**The numbers** (↑/↓ pick, ←/→ turn, SHIFT: five steps at a time):
- **Body:** length, width, height, nose taper, tail taper.
- **Wings and fins:** wing span (0: none), wing sweep, a tail fin.
- **Slots:** size class (1–4: how big its slots are), cargo racks (0–3), hardpoints (0–3),
  utility slots (0–2), drive nozzles across the tail (1–4).
- **Placement along the body:** where the thruster quads sit and how far apart, where the belly
  lift sits, and where the engine room (plant, drive, hyperdrive), the tank, the hold and the
  bridge (computers, life support) sit.

**What it's built into:**
- **The shape:** a hexagonal body tapering to the nose and tail, with wings and a fin as convex
  parts.
- **The nodes:** drive nozzles across the tail (drawn round), thruster quads, belly lift, landing
  gear, cockpit, and a mount for every slot where you placed it.
- **The frame:** its mass from its size (108 kg per m² of V^⅔), its strength and price from
  that, plus a price per slot size.
- **A stock fit:** the cheapest module that fits each slot, and a plant that covers the load.
  Refit it to taste in the planner.

**Nothing is balanced for you.** Where the masses sit and where the thrusters push from are
yours. The page shows the hull's numbers (lift in g empty and full, balance, endurance, price)
and draws it from above and the side, with the mounts and the centre of mass. It warns when
it's off:
- "CAN'T HOVER AT 1 G LOADED (SPACE ONLY)";
- "OFF BALANCE: LIFT KEEPS 25%".

A tail-heavy layout with the thrusters forward keeps a quarter of its lift.

**Commissioning (ENTER on the last row):**
- The design becomes a hull for good, numbered among yours (DESIGN 1, DESIGN 2…).
- Its key comes from its numbers, so the same design is the same hull wherever it's loaded.
- It appears on the HULLS page, the planner plans from it at once, and any station's shipyard
  builds it (SHIFT+ENTER on the PLAN page).
- Designs are kept in the save and commissioned again on load, before the ship built to one is
  read back.

**Under it:**
- The content registry takes entries added as the game runs (`Registry::add`, up to 256 of a
  kind, each fixed once set).
- A designed hull carries its own shape (`ClassSpec::shape_own`).
- `design::Design` builds and commissions. Dev scenarios `designer` and `designerbad`.

Tests:
- the default design builds a hull that flies, and commissions once, findable by its key;
- a ship built to a design saves by its key and loads as it;
- a tail-heavy layout keeps far less lift than a balanced one.
