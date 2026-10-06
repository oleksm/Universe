# People live by their needs

- **Each settlement's people take what their needs say** (`need.*`, per person per second) from
  their settlement's stores (life support and water works first, the warehouse last): a stock item,
  or a whole market category as any stock of it (food: any food). Where a world's air is breathable
  (Treistun d and e) they breathe it free. What they give back (used water, breath) goes to a works
  that takes it (the water works cleans used water), else the warehouse.
- **A need unmet adds up:** each keeps how long it has gone short. Past what it lasts (air 3
  minutes, water 3 days, washing 2 weeks, food 3 weeks, medicine 90 days), people die as fast as it
  goes unmet. Short of what keeps them alive, up to a third queue to leave. Needs that take no stock
  (warmth, shelter, home, safety...) aren't run here.
- **From the registry's day 0, the economy alone:** air and water hold everywhere; food lasts as
  long as the stores (30 days), then people start dying three weeks on. Port Eikir's farms make
  nothing: they're held by fertiliser, whose works takes nitrogen nothing supplies. Medicine,
  clothes, tools, art and company are supplied nowhere. Asked of the registry.
- `cargo run --release -p universe-sim --example needs_report`: each settlement's people and needs
  every ten days, two months on.
