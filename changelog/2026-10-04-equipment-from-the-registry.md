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
