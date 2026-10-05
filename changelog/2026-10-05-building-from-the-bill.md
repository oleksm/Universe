# Building from the bill of materials

- **Parts, equipment and hulls are stock** (878 items now), counted by the piece: a part, a
  product or a hull is one unit of its own mass (a hull's, its parts'). Bulk stock stays by the
  tonne. Prices are a unit's: worked out per kg through the recipes, `prices.ron` where it prices.
- **Shop modules have recipes, from the bill** (`world::recipes`): a part, at the module its record
  names, from what it is made of, what's cut away coming out as scrap of its metal; a piece of
  equipment or a hull, from its parts (`Registry::built_of`), at the module that makes it. The
  welding bay and the assembly shop can each be set to 246 things, the machining centre to 124,
  the building dock to the MC-07. The economy runs every module on these engine recipes.
- **Setting a module is an action** (`Economy::set_up`, `Universe::set_up`, recorded for replay):
  its works' owner, a player or a company alike, sets it to one of what it can make, or to nothing.
  A changeover is at once (its cost isn't described yet). Shop modules start idle: no starting
  setups are seeded.
- The registry crate remembers each part's folder; `built_of(product)` gives its parts with their
  counts (a hull's: the folder of its name, until hulls name theirs).
- **The MC-07 weighs what its parts do** (the user: "not being able to land should create some
  motivation"): a hull the registry describes by the model imported has its parts' mass and price.
  153.6 t dry fitted, not about 100. Its lift (1,320 kN) can't hold it up at Zaudalein, Eikir and
  Nacaubun (0.8 to 0.9 of its weight) or Lisaur (0.5); it can at Treistun's six lighter worlds.
- **A port's market sells ship modules and hulls from its warehouse.** Landed at a port with a
  market, a refit takes the module from what lies in the warehouse, at the exchange's ask, and the
  one taken out goes into the warehouse at its bid; a hull is bought only if a frame of it lies
  there (its stock fit still brought in). Stations sell as before, brought in from outside.
- **Stock is counted in units on the market:** tonnes of bulk stock, pieces of parts, products and
  hulls; one 144 kg part is on sale (it took a tonne before).
