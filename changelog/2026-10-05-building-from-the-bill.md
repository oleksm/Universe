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
- **Held, for the user:** the MC-07 flown at its parts' weight (153.6 t dry, not about 100). Its
  lift (1,320 kN) then can't raise it at Zaudalein, Eikir, Nacaubun or Lisaur.
