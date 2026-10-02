# The hold fills by space as well as weight

- **A hold has room as well as a weight limit:** its racks' volume (CARGO RACKS 20 T: 30 m³).
  Light goods (water ice at 0.6 t/m³, textiles at 0.35) fill it by space long before its
  weight; dense ore stops at the weight.
- **The ship keeps its cargo's volume** (`cargo_volume`, from each good's bulk density) beside
  its mass.
  - Buying checks both: "NO SPACE IN THE HOLD".
  - Mining stops when either is full (`Ship::takes`, by the ore's density).
  - A refit to a hold too small for what's in it is refused.
- **Traders plan their buys by both.**
- **The cargo screen** shows space used against the hold's room; the bar fills by whichever is
  fuller.
- **The planner** shows the hull's SPACE (its modules' volume against the hull's), the hold's
  room beside its weight, and passenger SEATS.
