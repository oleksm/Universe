# People live and die by what they're supplied

- **Each place has a "fed" level:** its food and water met against its people's need, over the
  last few days.
  - **Fed (over 95%):** it grows 0.2% a day, to three times its founding size.
  - **Hungry (under 90%):** people want to leave. They wait for passage (up to a third of
    them), 5% a day at worst; only a ship can take them.
  - **Starving (under half fed):** they die, up to 1% a day in full famine, those waiting
    included. A station left a month without deliveries: fed 0, 3,100 of its 25,000 dead,
    7,200 waiting to leave, 220 dying a day.
- **Works follow the workforce:** a place's works run in proportion to its people against its
  founding numbers (up to twice). A famine spreads into what a place makes; a growing place
  makes more.
- **Farm worlds have wells:** 60 t of water a day, enough to sell (their rain waters the fields
  too). Before, their people went thirsty.
  - Only Earth-like worlds (oceans and rain: temperate and oceanic) have water to draw.
  - Dry mining worlds import theirs, from them or from the outposts' ice works.
- **A place's storage keeps its founding size** (`Place::storage`): its warehouses don't shrink
  when its people do. (The stock limit had followed the shrinking target.)
- `Place::depart` and `arrive` move people between places (passengers, next).
- **The economy panel shows FED and WAIT per place,** and for the one picked, fed, waiting and
  growth or deaths a day.
- Test: unsupplied people go hungry, queue to leave, then die, while a fed farm world grows.
