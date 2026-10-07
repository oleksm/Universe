# The audit's bugs (phase 1)

- Works and the warehouse traded products at a tonne's price a kilogram over 1000: a 50 t hull
  paid 50 times its price. Paid a unit now (a tonne of bulk, a piece of a product); bulk as before.
  The market's docs said "a tonne" where they mean a unit.
- The player's trades are booked as any pilot's (`record_trade`): they count in the trade
  statistics now (they were logged but not counted).
- The environment's diffuse cube is gone: drawn every frame (6 of its 54 passes, 256 samples a
  texel) and read by no shader (the model shader's diffuse light is the scene's ambient). Its
  passes, texture, binding and shader branch removed; a frame of the docked deck is the same
  but for what moved.
- The player's dealings docked are in the input log (refuel, repair, refit, buying a hull, the
  vending machine, the drive's trim, passengers): they changed the ship and the ledger without a
  record, so a replay, or a save made from one, built a different world. Seven new `audit::Op`s,
  noted where the player does them and carried out on replay. The state hash now counts a ship's
  hull, fit, fuel, cargo and passengers; the recorded-session test buys a hauler at its start
  (and fails without the record).
