# A registry hull built as its record says

At the ships session's request (`docs/ships/registry-fit-request.md`, the user: build to match the
registry):

- **A hull the registry describes by its model is fitted as its record's `fit`,** not the cheapest
  module for each slot. The MC-07 no longer gets cargo racks in a slot its record leaves empty.
- **Its hold is its record's `capacity`** (an ore bay built into the frame, `HullFrame::bay`,
  beside any racks fitted) where its record fits no racks: the MC-07 holds 1,200 t and 669 m³,
  not 4 t and 6 m³.
- **Its dry mass follows:** 153.3 t (the racks' 0.35 t gone). Full, it can lift off nowhere heavy
  (a tenth of its weight at Zaudalein); the record means it to unload in orbit.
- Hulls the registry doesn't describe keep the stock fit; the stock hulls' records list their
  racks as before.
