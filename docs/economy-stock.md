# The economy on stock

*The engine's design for the economy after the user's decisions of 2026-10-05: only what the
registry describes runs (a hard switch), and markets trade stock, physically. It follows the
production model in `docs/registry-recipes.md`.*

## What goes

- The generated goods catalogue: 50 invented goods for each market category, their names drawn from
  adjectives and nouns.
- `content/base/recipes.ron` and `places.ron`: the four kinds of place (station, farm world,
  mining world, outpost) and their works.
- Markets made up from the seed where there is no economy behind them, and their bans.
- A place's people as a number per kind of place.

## What there is instead

**Stock.** Everything bought, sold, carried or stored is a registry item: a good (`good.bread`,
`good.bauxite`, the ores an excavator digs), a stock item (`stock.al6061-pl-5`), an element where
a recipe takes one (`element.o`). The catalogue is the registry's items, by key, in a fixed order.
A ledger line and a hold's cargo name one. Everything is counted in kilograms, and shown in tonnes.

**A facility is one pool of stock.** Each facility the registry places (a works at a port, a rig)
runs as one pool:

- its modules, each set to one of its recipes (its **setup**). The seeded setup is the one its line
  was built for: back from what the line makes, through its modules' recipes, as the registry's
  own build finds it. A shop module (welding bay, machining centre) is set to a part that names
  it.
- each module runs at its recipe's rate as far as the pool holds its inputs, the facility has
  power for it, and the facility's storing modules have room for what it makes. A full store stops
  what fills it.
- what it makes goes into the pool; the next module takes it from there. No transport inside a
  works.

**A settlement's market is its warehouse.** What is on the market lies in the warehouse the
exchange approved (Trethi Warehouse and the nine like it). A facility's owner sells what it makes
and does not use into the warehouse, and buys what it needs and does not make from it. A pilot
trades there the same way. Nothing moves between settlements but by ship.

**Prices are the game's own.** Prices are not in the registry. A market prices each stock item from:

- its reference price: `prices.ron` for what nothing makes (raw stock: ores, bauxite), and what
  goes into making it for the rest (inputs at their prices, power at the power price);
- how its stock stands against what is wanted of it: short is dear, a glut is cheap.

A stock exchange replaces this later.

**People.** What a person takes and gives is the registry's needs (`need.*`: food, water, air
and the rest, per second). How many people live at a settlement is not in the registry yet. Until
it is, a settlement has none, and nothing is eaten.

**The exchange makes a market in any stock** while its warehouse has room (the user, 2026-10-05):
what the settlement's works take, at the prices above; anything else, at half its reference price
with the warehouse empty, falling to nothing as it fills. So a miner can always sell, and what it
sells lies in the warehouse for anyone to buy.

**Brought in from outside the economy, for now:** fuel where no warehouse holds any (at the old
fuel price), a refit's modules, a new hull, a repair's metals. Each waits for the registry to
describe where it comes from.

## What stops for now (accepted)

- Every place the registry doesn't describe has no market: the stations, the other systems'
  ports, the frontier. Treistun's ten ports trade.
- Food, people, passengers and migration wait for settlements with populations, farms and
  hydroponic halls. (The passenger mechanism stands, for when there are people.)
- Port Trethi's chain needs bauxite, caustic soda and anodes that nothing makes yet, and its power
  station needs deuterium. Until the registry gives starting stock, it runs only as far as what ships
  bring in.
- Shop modules (welding bay, machining centre, building dock) are set to nothing: a part is the
  owner's choice, and no owner chooses yet.
- Market bans are gone: they were rolled from the seed. Law will set them.

## Asked of the registry

- A **population** for each settlement.
- **Starting stock** for the seeded world, if the world is to start other than empty: what lies in
  each warehouse and each works' store at day 0.
- The other settlements, with their facilities.
