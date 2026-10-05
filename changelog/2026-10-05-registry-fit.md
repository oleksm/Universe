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

## Parts made of parts

- **A part made of parts** (the MC-07's landing legs, cargo ramp, clamps and mining laser: no
  mass of their own, a folder named by their code, `parts/mc-07/MC07-23/`) is built at its module
  from those parts and weighs what they do, by their counts. `Registry::built_of` finds a part's
  parts by its code, and a hull's by its `built_of.parts`.
- **The MC-07's frame is 154.2 t** (its record's 154), 165.6 t dry fitted. Empty, its lift is 0.83
  of its weight at Zaudalein, 0.74 at Eikir, 0.81 at Nacaubun, 0.48 at Lisaur; 1.91 at Trethi.
- **Companies plan down the bill:** a yard wants each thing its hull and fit take, and what goes
  into those, to the bottom (it set nothing to the legs' own parts before). Trethi Yard builds an
  MC-07, legs and all, in 23.9 days from its day-0 stock.
