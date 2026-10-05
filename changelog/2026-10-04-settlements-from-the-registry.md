# Settlements and industry read from the registry

- **Every settlement's ground comes from Local Administration's records** (`settlement.*`, `zone.*`,
  `parcel.*`, `street.*`, `power-line.*`, `facility.*`). The industrial modules come from the SFO's
  (`module.*`) with their recipes. `settlements.ron` and `industry.ron` are no longer loaded; all
  eight files the registry's build wrote for the game are now read from the records instead.
- **The engine works out what a facility can do at most** (`settlements::line_most`), as the build
  did. Back from what a line makes, it finds the recipe each module is set to. The line is held to
  what its tightest module lets through. From that come what it takes in and gives off, the power it
  draws, what a power station supplies and burns, and what a store holds. The figures are the
  build's own: every power figure, store, flow of goods and module layout is the same as it was.
- **Two differences, both shown, not used in play.** A line of shop modules (welding bays,
  machining centres, the assembly shop, the building dock) is named after its modules, where it was
  "Parts" or "Hulls". Those lines no longer list a "Parts" or "Hulls" output: the registry has no
  such goods any more.
- **Parcel owners are organisations by their registry keys** (`org.castellan`).
- **A maker's home moved.** Where each maker's whole range is sold is drawn from the galaxy's seed
  and the maker's key, and the keys changed with the last step (`brand.hadley` is `org.hadley`), so
  the draw came out differently. A new world has its makers' homes in other settled systems than
  before.
- **A maker's home is where its record says it is**, not a draw from the seed. Its address names a
  settlement, the settlement its body, the body its system. All twelve makers are at Treistun e
  station today, so each one's whole range is carried at Treistun, and less of it, dearer, the
  farther a station is in gate hops.
