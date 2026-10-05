# Ship equipment read from the registry

- **What hulls and structures are fitted with is the registry's equipment** (`equipment.*`, SFO
  16): 56 pieces, each with its maker, size class, mass, volume, power draw and its `function` (a
  drive's thrust, a tank's capacity, a cabin's seats...). `modules.ron` is gone. The figures are the
  same as before; message rates are held per second in the records, so a comm's capacity can be
  0.000000001 off.
- **Equipment is known by its registry key** (`equipment.drive.torch.s2`), in hulls, structures and
  the code.
- **Prices are the game's own**, in `content/base/prices.ron` by registry key, until a stock
  exchange sets them (prices are volatile, never the registry's). The registry's throat coil is
  left out: the game doesn't make one yet.
- **Fuels are the registry's materials** (those with a `fuel` group: how they give up their energy
  and how much). `materials.ron` is gone. Three are spelled as the registry has them now:
  `material.helium-3`, `material.d-he3`, `material.d-t`.
- **Structures are the registry's:** the platform, spaceport, outpost and orbital site
  (`structure.*`) and the three gate rings (`gate.*`, span and capture speed from the ring's
  record). `structures.ron` is gone. A ring is `gate.ring.i` now, not `structure.ring.i`. The
  rings' throat coils (60 to a ring in the records) aren't fitted in the game: it doesn't make them
  yet.
- **The five stock hulls are the registry's** (`hull.drover`, `.sprint`, `.hauler`, `.prospector`,
  `.interceptor`): frame, slots, fit, nozzles, radius, drag and strength from their records, the
  same as before. `hulls.ron` is gone; their prices are in `prices.ron`. The MC-07 is still built
  from its model by the importer: moving it to its record changes how it flies (its mass from its
  parts, its ore bay), and that waits on the user.
- **Goods are the registry's market categories** (`market.*`): name, unit mass, bulk density,
  household basket and the words their goods are named from, as before. The game's kinds of goods
  are keyed by them now (`market.food`, not `goods.food`). Their price ranges are in `prices.ron`.
  `goods.ron` is gone.
- **The generated catalogue is drawn by each category's key, not its place in a list**, so adding
  a category changes no other's goods. A new world has different goods names and prices than
  before.
- **Ores are the registry's rock goods** (`good.stony-ore` and the rest): name and bulk density
  from their records, prices in `prices.ron`. `ores.ron` is gone. What a rock class yields names
  the ore directly.
