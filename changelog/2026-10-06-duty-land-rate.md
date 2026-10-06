# Duty and the land rate, from the law

- **Duty on sales** (`policies.duty`, Treistun 2%): every sale at a place under a law pays its
  share to the administration (`Party::Administration(system)`), the seller paying: the warehouse
  selling to a works or a pilot, a works' owner selling to the warehouse, a pilot selling to the
  market (what a pilot is paid for a sale is net of it), fuel sold at a market. `Place::duty`, from
  `world::order::law` by the place's system. A rig in a system with law pays it too (the law's
  jurisdiction is the whole system).
- **The land rate** (`policies.land_rate`, Treistun 1% a year): `LandOffice::levy`, daily: each
  owned lot's owner pays the rate on the lot's worth (its area at the office's price) to the
  administration.
- Not duty: power sold between works.
- Test: the trade test checks the duty leg and a day's levy.
