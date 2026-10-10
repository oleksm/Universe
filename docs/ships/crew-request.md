# Request to the Scientist: crewing a ship (the pilot's station and life support)

*From the ships session on `ships`, 2026-10-06. The user's ask: "From pilot view. Life support
features ... manage request with registry". The interior studio's design-2 has six passenger seats
and nothing else for people: no pilot's seat, no bunk, no galley, no food. Below is what the studio
needs from the registry, in the user's order. Each item says what record, which figures, and the
check the studio will run on it. Keys and field names are yours; tell me them and I'll read them
through the generated types, as with the panels and nodes.*

## 1. Life support capacity (a gap today)

`equipment.life.s1`'s own basis says "800 kg keeps about three people" (by the space station's
weights), but nothing on the record says how many. design-2 fits one unit for six seats and the
studio can't tell.

- **Asked:** `function.persons` on each life support record (how many people it keeps), with its
  tier. A larger unit if three is the ceiling (S2 exists: what does it keep?).
- **Check:** life support units × persons ≥ crew aboard; short: a fault.

## 2. The command station

- **Asked:** equipment, slot kind of your choosing (`command`?), in two or three sizes: a pilot's seat
  and console (S1), pilot and co-pilot (S2), a bridge of four (S3). Each: mass, size, power drawn,
  heat, `persons` (who it seats), the most g its seats are rated for, and which way they face
  (eyeballs-in to the thrust is the usual), a mount, parts and a chain.
- **Viewports** exist (S1 0.5 m², S2 2 m²); nothing more needed for them.
- **Checks:** a station on the bridge, inside the pressurised space; its seats' g rating against the
  design's landing and thrust g; a sightline from the pilot's eye point (the station's, or a DASH point)
  to the landing pads through a viewport or a camera; the flight and nav computers inside the
  pressurised space.

## 3. Crew quarters, galley, sanitation, food

The cabins are all "Passenger cabin N". A crewed ship needs somewhere to sleep and eat.

- **Asked:**
  - **Crew berths:** a bunk room for 2, 4 and 6 (mass, size, `persons`).
  - **Galley:** mass, size, power.
  - **Head:** toilet and washing (mass, size, water use, or say it's in the needs).
  - **Medical bay:** later is fine.
  - **Food store:** a `store` that `holds: market.food` (or the good you use), capacity in kg, with
    its mass and size, as the water tank is.
- **Already on record, used as is:** `need.food` (1.5 kg a head a day).
- **Checks:** berths ≥ crew (crew = command station seats + berths' crew; passengers separate);
  food days, as air and water.

## 4. Leaks, airlock losses, cabin cooling

- **Asked:**
  - A hull **leak rate**: the design figure for how much air a pressurised space loses (kg a day per m³,
    or per m² of wall). Stations lose about 0.1–1 kg a day; a source if you have one.
  - Whether **people's heat** (about 100 W a head) belongs on `need.air` or a need of its own.
  - **Cabin air conditioning:** the cabin's own cooling and dehumidifying, on life support (a
    figure: W it carries) or a separate unit.
- **Already on record, used as is:** the crew airlock's `air_lost` (1 kg a cycle).
- **Checks:** the air budget counts leaks and airlock cycles (cycles a day, chosen in the studio);
  the cabin's heat against what carries it out.

## 5. Landing aids

- **Asked:** a **landing altimeter / lidar** (range, accuracy, mass, power) and **cameras** (field of
  view, mass, power): sensors a pilot lands by. The radar's 500 km is not for the last 100 m.
- **Check:** a ship that lands fits a landing sensor looking down, clear of its legs.

## 6. Safety: fire, compartments, suits, radiation

- **Asked:**
  - **Fire detection and suppression:** a unit per room volume, its mass.
  - **Pressure door:** a hatch that holds cabin pressure across it (mass by size). The studio's doors
    then split the ship into compartments.
  - **Suit and suit locker:** mass, size, hours of air.
  - **Radiation shielding:** a material and areal mass for a storm shelter (water and polyethylene are
    the usual), and the dose figures to size it by; later is fine.
- **Checks:** a fire unit in each pressurised room; compartments (a hole vents one, not all);
  suits ≥ crew; shelter volume for the crew.

## Already fine, no request

Oxygen and CO₂ as one air-recovery figure (0.5), water recovery (0.9), the water tanks, the waste
tank, the battery bank (the studio will check hours on battery), reaction wheels, comms, the
transponder.
