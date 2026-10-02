# Each ore its own density; some people always move on

- **Each catalogue item has a bulk density** (t/m³ as stowed in a hold): generated goods their
  kind's, ores their own (in `ores.ron`), the solid rock's less the gaps between the broken
  pieces:
  - water ice 0.6;
  - carbonaceous 1.2;
  - stony 1.8 (0.56 m³ a tonne; it was 2.2, the ores' shared figure);
  - PGM-rich 4.0;
  - nickel-iron 4.5.
  The cargo screen's volumes follow. (Holds are limited by mass for now, not by volume.)
- **Some people always want to move on:** 0.05% a day of any place joins those waiting for
  passage, fed or not, no more than 2% waiting at once. Hunger multiplies it, up to 5% a day and
  a third of the people. Fed again, most of those waiting stay.
