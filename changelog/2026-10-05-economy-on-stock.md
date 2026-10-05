# The economy on stock

The user's decisions: only what the registry describes runs (a hard switch); markets trade stock,
physically; the exchange makes a market in any stock while its warehouse has room, and the
registry is to give starting stock. Design: `docs/economy-stock.md`.

- **Stock is the registry's.** The catalogue is every good, stock item, and element or material a
  recipe names (217), by key; the 50 invented goods for each category are gone. Each is traded by the
  tonne. Prices are the game's: `prices.ron` for what nothing makes, otherwise worked out from what
  goes into making it (inputs at their prices, power at 40 cr/MWh, a quarter over). Unpriced raw
  stock is 100 cr/t; deuterium is priced at the old fuel price until its recipe draws water in place.
- **A facility is one pool of stock.** Each registry facility runs its modules, each set to the
  recipe its line was built for, as far as its pool holds their inputs, the port's power stations
  (burning their fuel) supply them, and its stores have room. A full store stops what fills it.
- **A settlement's market is its warehouse.** Works sell what they make and don't use to it, and
  buy what they take from it to cover ten days. Pilots trade there; every trade moves stock in or
  out. What the works take is priced by how the stock stands against ten days of it; anything else
  the exchange buys at half its price, less as the warehouse fills.
- **Only Treistun's ten ports have markets.** Stations and other systems have none. Fuel where no
  warehouse holds any, refits, new hulls and repairs are brought in from outside the economy, at the
  old prices. Miners sell at a port with a warehouse.
- **Gone:** `recipes.ron`, `places.ron`, `markets.ron` (bans), the kinds of place, people as a
  number per kind (no settlement has a population in the registry yet: no passengers for now), the
  coarse economy's examples.
- **The economy panel** lists the settlements, each one's works (rate, what held it back), and its
  market (stock held, what its works take, make and use a day, ask and bid). **The market screen**
  lists stock by the tonne: kind, sells or buys, ask, bid, stock or room, held.
- Dev scenarios `market`, `marketnear`, `marketfar` and `economy` stock Port Trethi's warehouse
  first (none is seeded yet); `economy` gives its works a day of what they take.
- **Goods' categories from their `traded_as`** (merged from the registry the same day): the game
  read the old `game.goods`; content loading now refuses an ore traded as nothing.
