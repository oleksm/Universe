# The world's figures still in Rust: what the registry is to take

*From the integration session to the registry session, 2026-10-04. Your item 8: the values the
game's code holds that describe the world, not how the engine works. When a group is in records,
I switch the code to read them and delete the constants: the generated types load them with no
change on my side. Prices stay the game's (`content/base/prices.ron`).*

Already out of the code: Dogma's laws, the galaxy's settings, rock classes, the charted systems,
equipment, hulls, structures, fuels, goods, ores, settlements and industry.

## How to read this

- **Seeding** is how the seed makes things: it goes in `seeding` records, beside `seeding.galaxy`
  and `seeding.asteroids`. Most of it is the game's own choice (invented), as the code's comments
  say; record it as such. Where the registry already describes the real thing differently
  (`seeding.asteroids` is the Sun's belts; the game makes swarms round a remnant), record what the
  game does now, under its own name, so the two can sit side by side until the engine follows the
  spec.
- **A product's figure** (a gun's muzzle speed, an excavator's power) goes on its `equipment`
  record's `function`, as the branded-products rule says.
- **Rules of a place** (the land office's rates, how a population grows) go on the administration
  or the settlement.
- **Not here:** engine tuning that stays in code (rendering, level of detail, physics steps, the
  network, the solver's weights, avionics, the UI), and NPC behaviour (pirate odds, the minimum
  profit a trader wants), which the no-intentions rule keeps in the NPC client.

## The generators' figures written inline

These have no names in the code: they are numbers inside the functions that make a system. They
are the largest group, and they decide what every unexplored system looks like.

**`world/src/system.rs`, `StarSystem::generate` and what it calls:**
- **The star:** its mass and radius ±15% of its class's.
- **Orbits:** the frost line at 2.7 AU·√L; the first planet at 0.25 AU·√L; each next ×1.5 to 2.1.
- **Rocky planets:** mass 10^(-1.3…0.7) Earth masses, radius ∝ mass^0.28; temperate where
  |ln(a / habitable distance)| < 0.4.
- **Gas giants:** a 0.6 chance; 8–12 Earth radii, 40–1,000 Earth masses; rings 45% of the time.
- **Ice giants:** 3.5–4.3 Earth radii, 10–25 Earth masses; rings 20% of the time.
- **Eccentricity, day and tilt:** 0 to 0.07, the day lengths, tilts of 7° and 30°.
- **Moons:**
  - density 3,000 kg/m³ for every moon, icy or not (the code says this is a simplification);
  - the first at 3–6 planet radii, out to 0.35 of the Hill radius;
  - each next ×1.4 to 2.0;
  - radius 0.12–0.3 of the parent's, or 0.08–0.45 Earth radii;
  - eccentricity 0 to 0.02.

  These are the moons that come out too close and too stretched (your tidal heat finding): the
  first-moon distance and eccentricity are where to fix it.
- **A station:** 1×10⁹ kg, 600 m radius. **A gate:** 1×10¹⁰ kg. **`GATE_CLEARANCE`**: 100 km.
- **Body colours:** the palettes rocky, terran, gas, ice, moon (`system.rs`, the `*_COLORS`
  tables). These belong to each vocabulary kind.

**`world/src/belt.rs`:**
- **Belt edges:** 0.40 and 0.63 of the first giant's orbit; or 0.8 and 1.3 of the frost line with
  no giant (that pair is in `seeding.asteroids` already).
- **Kirkwood gaps:** 0.4807, 0.5428 and 0.5703 (the 7:3 gap is truly 0.5684), each ±0.007.
- **A family's class:** stony 0.55, then carbonaceous to 0.8, metallic the rest, by zone (the
  odds at 0.7, 0.85, 0.95).
- **Trojans:** a 0.8 chance a side for a gas giant, 0.35 for an ice giant, exactly at 60°, half
  carbonaceous and half icy.
- **The outer belt:** 1.31–1.6 times the outermost giant's orbit, inclined up to 20°.
- **Spin:** a rubble pile turns in 2.3–30 h, a monolith in 0.05–10 h.
- **Structure:** a metallic body under 200 m is a monolith; one over, half the time.

**`world/src/galaxy.rs`, `shape_stars`:**
- the spiral's own seed 1984 (separate from the world's);
- arms at a pitch of 13°, running at angle ln(r/300);
- the disc 150 ly thick, sampled with 40,000 stars;
- `REGION_CENTRE` (−900, 0, 3,900) ly.

**`world/src/terrain.rs`:**
- relief amplitude: radius × 0.0007, held to 1.5–7 km;
- craters by terrain kind: terran 4, dry 25, cratered 70;
- and the named constants in the table below.

**`world/src/goods.rs`:** each generated good's mass ×0.6 to 1.4 of its category's unit mass.
That's seeding for the market categories.

## The named constants

Every named figure of the kinds above, as the code has it today, with its comment.

### Seeding: how the seed makes systems, belts and ground (→ `seeding` records)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `SHAPE_CELLS` | 256 | `world/src/galaxy.rs:173` | Cells a side of the shape grid, and how far it reaches (ly, each way from the centre). |
| `SHAPE_REACH` | 10_500.0 | `world/src/galaxy.rs:174` |  |
| `GATE_CLEARANCE` | 100_000.0 | `world/src/system.rs:183` | A gate's path keeps at least this clear of anything else orbiting with it (m): its run-in and the way out are open space. |
| `SMALLEST` | 15.0 | `world/src/belt.rs:37` | Smallest fragment in a swarm (m across). |
| `SWARM_MAX` | 60_000.0 | `world/src/belt.rs:39` | A swarm reaches at most this far from its remnant (m)... |
| `SWARM_HILL` | 0.3 | `world/src/belt.rs:42` | ...and no farther than this share of the remnant's Hill radius (orbits out there are stable against the star's tides). |
| `PAD_FLAT_INNER` | 4_000.0 | `world/src/terrain.rs:14` | Flat around a spaceport out to this ground distance, blending back to natural terrain by `PAD_FLAT_OUTER` (m). |
| `PAD_FLAT_OUTER` | 40_000.0 | `world/src/terrain.rs:15` |  |
| `RELIEF_OCTAVES` | 8 | `world/src/terrain.rs:21` | The small-scale relief (see `Terrain::relief`): its octaves, the first's frequency (a feature about 1/RELIEF_FREQ radii across: 25 km on ... |
| `RELIEF_FREQ` | 300.0 | `world/src/terrain.rs:22` |  |
| `RELIEF_LACUNARITY` | 2.1 | `world/src/terrain.rs:23` |  |
| `RELIEF_SLOPE` | 0.035 | `world/src/terrain.rs:24` |  |
| `RELIEF_ROUGHEST` | 1.6 | `world/src/terrain.rs:25` |  |
| `RELIEF_FLAT_INNER` | 20_000.0 | `world/src/terrain.rs:27` | The plain round a spaceport: no small relief within this (m), all of it by that. |
| `RELIEF_FLAT_OUTER` | 60_000.0 | `world/src/terrain.rs:28` |  |

### Structures and their places: ports, pads, stations, traffic (→ `structure` records, or a standard)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `PAD_RADIUS` | 400.0 | `world/src/spaceport.rs:10` | Touching down within this distance of the port's center counts as landing at the port (m): the whole pad grid, its corners included. |
| `LAND_SPEED` | 30.0 | `world/src/spaceport.rs:12` | Touching a surface slower than this lands instead of crashing (m/s). |
| `GRID` | 4 | `world/src/spaceport.rs:15` | Landing pads per spaceport, in a square grid (`GRID` × `GRID`), this far apart (m). |
| `PADS` | GRID * GRID | `world/src/spaceport.rs:16` |  |
| `PAD_SPACING` | 150.0 | `world/src/spaceport.rs:17` |  |
| `CENTER_PAD` | GRID + 1 | `world/src/spaceport.rs:19` | A pad next to the grid's middle (where a ship starts). |
| `PAD_SIZE` | 45.0 | `world/src/spaceport.rs:21` | A ship on the ground within this of a pad's center is on that pad (m). |
| `DECK_HALF` | 300.0 | `world/src/station.rs:20` | The deck's half-width across (local X) (m). |
| `DECK_TOP` | -100.0 | `world/src/station.rs:22` | The deck's top, and the station's bottom, along local Y (m). |
| `BOTTOM` | -150.0 | `world/src/station.rs:23` |  |
| `DECK_FROM` | -225.0 | `world/src/station.rs:25` | The pad area along local Z (m): from the main structure's face out. |
| `DECK_TO` | 375.0 | `world/src/station.rs:26` |  |
| `STRUCTURE_FROM` | -375.0 | `world/src/station.rs:28` | The main structure: from the far edge to the deck, and how high (m). |
| `STRUCTURE_TOP` | 150.0 | `world/src/station.rs:29` |  |
| `STATION_SIZE` | 500.0 | `world/src/station.rs:31` | About the station's size: its bounding radius, rounded (m). |
| `DECK_SPEED` | 10.0 | `world/src/station.rs:33` | Touching the deck slower than this lands (m/s); faster wrecks. |
| `BUMP_SPEED` | 15.0 | `world/src/station.rs:35` | Hull contact slower than this bounces instead of destroying the ship (m/s). |
| `DOCK_RANGE` | 50_000.0 | `world/src/traffic.rs:15` | Docking clearance is granted within this range of the station (m). |
| `LAND_RANGE_RADII` | 20.0 | `world/src/traffic.rs:17` | Landing clearance is granted within this many planet radii of the pad. |
| `TRANSIT_RANGE` | 50_000.0 | `world/src/traffic.rs:19` | Transit clearance is granted within this range of the gate (m). |

### A person (→ a new `person` or species record)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `EYE` | 1.7 | `world/src/crew.rs:28` | Eye height above the feet (m). |
| `WALK` | 1.6 | `world/src/crew.rs:30` | Walking and running speed (m/s). |
| `RUN` | 4.5 | `world/src/crew.rs:31` |  |
| `JUMP` | 3.0 | `world/src/crew.rs:33` | Take-off speed of a jump (m/s). |
| `REACH` | 1.6 | `world/src/crew.rs:35` | How close to something you have to be to use it (m). |
| `BOOTS` | crate::units::STANDARD_GRAVITY | `world/src/crew.rs:37` | What the boots hold you to a floor with, aboard in flight (m/s²: as a world's gravity). |
| `CLIMB` | 1.0 | `world/src/crew.rs:39` | Climbing a ladder (m/s). |
| `STAIR_RUN` | 8.0 | `world/src/crew.rs:123` | How far aft a stair from a hull's own `hatch` runs to the ground (m). |

### Hulls and devices (→ `hull`, `equipment` and `material` records; the outdated design rules for the hull standard)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `FRAME_PER_AREA` | 108.0 | `world/src/design.rs:25` | The frame's mass per square metre of its size (V^⅔: its skin, roughly), kg. |
| `PRICE_PER_KG` | 3.0 | `world/src/design.rs:27` | What a kilogram of frame costs, and a size of slot (credits). |
| `PRICE_PER_SLOT_SIZE` | 2000.0 | `world/src/design.rs:28` |  |
| `STRENGTH_PER_KG` | 600.0 | `world/src/design.rs:30` | The energy that wrecks the hull per kilogram of frame (J). |
| `PASSENGER_MASS` | 100.0 | `world/src/ship.rs:137` | A passenger's mass (kg), with their things (their seat is a cabin's). |
| `SHIP_RADIUS` | 12.0 | `world/src/ship.rs:683` | A ship's size as traffic lays out room for it (pads, docking slots, corridors), and the starting hull's collision radius (m). |
| `TAXI_SPEED` | 15.0 | `world/src/ship.rs:1304` | How fast a ship taxis on the ground (m/s). |
| `TURN_RESPONSE` | 0.12 | `world/src/ship.rs:1308` | How quickly the flight computer brings the turn to the rates asked (s), as far as the thrusters allow. |
| `GUN_MUZZLE` | 3_000.0 | `world/src/weapons.rs:27` | Slug speed from the muzzle, relative to the ship (m/s). |
| `GUN_RATE` | 10.0 | `world/src/weapons.rs:29` | Rounds per second while the trigger is held. |
| `SLUG_MASS` | 0.5 | `world/src/weapons.rs:31` | Mass of one slug (kg). |
| `GUN_AMMO` | 500 | `world/src/weapons.rs:33` | Rounds in a full magazine. |
| `SLUG_LIFETIME` | 10.0 | `world/src/weapons.rs:35` | Seconds a slug flies before it's no longer tracked (about 30 km). |
| `LASER_POWER` | 2.0e6 | `world/src/weapons.rs:54` | Beam power on target at up to `LASER_FOCUS` (W). |
| `LASER_FOCUS` | 2_000.0 | `world/src/weapons.rs:56` | Beyond this the beam spreads: power falls as (focus / range)^2 (m). |
| `LASER_RANGE` | 30_000.0 | `world/src/weapons.rs:58` | Longest reach of the beam (m). |
| `LASER_BURN` | 8.0 | `world/src/weapons.rs:60` | Seconds of continuous fire from cold to too hot, and back. |
| `LASER_COOL` | 4.0 | `world/src/weapons.rs:61` |  |
| `LASER_RESET` | 0.3 | `world/src/weapons.rs:63` | After overheating, the laser is locked out until it has cooled to this. |
| `GIMBAL_LIMIT` | 4.0 * std::f64::consts::PI / 180.0 | `world/src/weapons.rs:66` | How far the gun's gimbal swings off the nose (rad): 4°. |
| `GIMBAL_RATE` | 30.0 * std::f64::consts::PI / 180.0 | `world/src/weapons.rs:68` | How fast it swings (rad/s): 30°/s. |
| `ON_TARGET` | 0.05 * std::f64::consts::PI / 180.0 | `world/src/weapons.rs:70` | The gun is on a direction when it points within this of it (rad): 0.05°. |
| `ARM_TIME` | 2.0 | `world/src/weapons.rs:109` | Seconds from the master arm going on to the weapons being hot. |
| `MISSILE_ACCEL` | 60.0 | `world/src/missiles.rs:20` | The motor's push (m/s², about 6 g), and how long it burns (s): 3.6 km/s all told, a hundred kilometres' reach. |
| `MISSILE_BURN` | 60.0 | `world/src/missiles.rs:21` |  |
| `MISSILE_LIFETIME` | 150.0 | `world/src/missiles.rs:23` | Gone after this long (s). |
| `MISSILE_FUSE` | 40.0 | `world/src/missiles.rs:25` | The proximity fuse (m), and the blast's energy at the target (J). |
| `MISSILE_BLAST` | 15.0e6 | `world/src/missiles.rs:26` |  |
| `LAUNCH_RELOAD` | 10.0 | `world/src/missiles.rs:28` | A launcher fires once every this long (s), and keeps at most this many in the air. |
| `IN_FLIGHT` | 2 | `world/src/missiles.rs:29` |  |
| `MISSILE_RANGE` | 80_000.0 | `world/src/missiles.rs:31` | How far a gunner launches at (m). |
| `NAV_GAIN` | 3.0 | `world/src/missiles.rs:33` | Proportional navigation's gain. |
| `TURRET_RANGE` | 6_000.0 | `world/src/turrets.rs:21` | How far a turret reaches (m): twice a ship's gun range. |
| `PORT_TURRET_RANGE` | 20_000.0 | `world/src/turrets.rs:24` | A spaceport's or a station's turrets reach farther: SAMs covering the approach and the holding circle (their rounds fly 30 km). |
| `TURRET_RATE` | 5.0 | `world/src/turrets.rs:26` | Rounds per second. |
| `TURRET_ID` | 1 << 40 | `world/src/turrets.rs:28` | Turrets' ids in combat start here (ships' are small numbers). |
| `TURRET_SLEW` | 4.0 | `world/src/turrets.rs:108` | How fast a turret's gun turns to its aim (rad/s). |
| `RADAR_RANGE` | 500_000.0 | `world/src/radar.rs:12` | How far the radar sees (m). |
| `EXCAVATOR_POWER` | 300_000.0 | `world/src/mining.rs:35` | The excavator's power (W)... |
| `EXCAVATOR_THROUGHPUT` | 10.0 | `world/src/mining.rs:37` | ...and the most spoil it can carry off (kg/s). |
| `RUBBLE_ENERGY` | 2_000.0 | `world/src/mining.rs:40` | A rubble pile, gravel held by its own weak gravity, scoops up at this (J/kg), whatever it's made of. |
| `ANCHOR_REACH` | 30.0 | `world/src/mining.rs:68` | The anchor reaches this far from the hull (m)... |
| `ANCHOR_SPEED` | 0.5 | `world/src/mining.rs:70` | ...and holds if the ship drifts slower than this against the surface (m/s). |
| `CLOSE_STANDOFF` | 12.0 | `world/src/mining.rs:72` | Closing on a rock to anchor, a ship holds this far off its surface (m). |
| `RESTITUTION` | 0.3 | `world/src/mining.rs:74` | Share of the closing speed kept in a bounce off a rock. |
| `IMPACT` | 0.3 | `world/src/mining.rs:76` | Slower than this, touching a rock just eases the ship off it (m/s). |
| `ABLATION` | 0.05 | `world/src/heat.rs:25` | Share of the heat beyond the limit that goes into damaging the hull (the rest is carried off by what burns away). |

### Administration: the land office (→ the administration's `org` record, or an office record)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `LAND_PRICE` | 2.0 | `services/src/land.rs:22` | What the administration asks for land (credits per m²). Invented, to be tuned with the economy. |
| `MODULE_PRICE` | 0.5 | `services/src/land.rs:25` | What a module costs to build (credits per m³ of it: materials and work, bought from outside the game's economy for now). Invented. |
| `BUILD_RATE` | 50_000.0 / 60.0 | `services/src/land.rs:29` | How fast a facility goes up: m³ of module a second, one module after another (one crew). Invented: an ore yard in 6 minutes, a shaft furn... |
| `REACH` | 4_000.0 | `services/src/land.rs:32` | The ground round a port that is flat enough to build on (m): the terrain's own (`terrain::PAD_FLAT_INNER`). |
| `POWER_PRICE` | 40.0 | `services/src/land.rs:35` | What a facility pays for power, to whoever supplies it (credits per MWh). Invented, to be tuned with the economy. |
| `LEAST_SIDE` | 50.0 | `services/src/land.rs:37` | A claim's least side (m). |
| `STREET_HALF` | 10.0 | `services/src/land.rs:39` | Paving each side of a street's line (m). |
| `AGGRESSION` | 600.0 | `services/src/law.rs:12` | How long a ship stays aggressed after hitting one that wasn't (s), where a holder's law doesn't say: 10 minutes. |
| `KEEP` | 1000 | `services/src/law.rs:44` | Rulings kept on record. |
| `MOST` | 100.0 | `sim/src/standing.rs:25` | Standing's bounds. |
| `AGGRESSION` | -10.0 | `sim/src/standing.rs:27` | Opening fire on the innocent in its system. |
| `MURDER` | -30.0 | `sim/src/standing.rs:29` | Destroying the innocent in its system. |
| `BOUNTY` | 5.0 | `sim/src/standing.rs:31` | Destroying the fair game in its system. |
| `TRADE` | 0.2 | `sim/src/standing.rs:33` | A trade at its market. |
| `HOSTILE` | -50.0 | `sim/src/standing.rs:35` | At or below this its turrets treat the pilot as fair game. |
| `FAIR_GAME` | 600.0 | `sim/src/standing.rs:38` | The law, in every settled system (one with a station or a port): one who opens fire on the innocent there stays fair game this long (s). |
| `EVERY` | 5.0 | `sim/src/standing.rs:40` | How often the authorities take in what they've heard (s). |

### Economy: population, markets, commerce (→ settlement and market records; prices stay the game's)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `STEP` | 600.0 | `services/src/economy.rs:28` | The economy steps this often (game s). |
| `COVER_DAYS` | 10.0 | `services/src/economy.rs:30` | Stock a place aims to hold: this many days of what it uses or makes. |
| `STORAGE` | 3.0 | `services/src/economy.rs:32` | Its storage holds this many times that; full, its works stop. |
| `GROWTH` | 0.002 | `services/src/economy.rs:35` | Fed (its food and water met, over a few days): it grows this much a day, to `ROOM` times its founding size. |
| `ROOM` | 3.0 | `services/src/economy.rs:36` |  |
| `RESTLESS` | 0.0005 | `services/src/economy.rs:39` | A few of its people want to move on anyway, fed or not: this share a day joins those waiting for passage (no more than `WAITING_CALM` of ... |
| `WAITING_CALM` | 0.02 | `services/src/economy.rs:40` |  |
| `EMIGRATE` | 0.05 | `services/src/economy.rs:42` | Hungry, many more: at worst this share a day (no more than `WAITING_MOST`). |
| `WAITING_MOST` | 0.33 | `services/src/economy.rs:43` |  |
| `DEATH` | 0.01 | `services/src/economy.rs:45` | Starving (under half fed), at worst this share of its people die a day. |
| `FED_DAYS` | 3.0 | `services/src/economy.rs:47` | How long being fed or hungry takes to tell (days). |
| `RECOVERY` | 6.0 * 3600.0 | `services/src/market.rs:36` | Time for stock and demand to get most of the way back (game s). |
| `BUY_BACK` | 0.7 | `services/src/market.rs:38` | A market buys back what it produces at this share of its selling price. |
| `GENERAL_PREMIUM` | 1.05 | `services/src/market.rs:72` | What a market pays for unlisted goods of a category it wants, over their worth. |
| `LOT_VALUE` | 8_000.0 | `services/src/market.rs:74` | Worth of a market's usual stock or demand of one line (credits). |
| `FUEL_PRICE` | 60.0 | `services/src/market.rs:130` | Ship fuel's usual price (credits a tonne), and what the frontier asks over it. |
| `FRONTIER` | 2.0 | `services/src/market.rs:131` |  |
| `VARIETIES` | 3 | `services/src/market.rs:134` | Varieties of each kind of goods a settled market lists. |
| `MAKER_PRICE` | 0.75 | `services/src/market.rs:167` | Against the catalogue price, what a settled place prices goods at where it makes them (cheap) and where it needs them (dear): the differe... |
| `USER_PRICE` | 1.3 | `services/src/market.rs:168` |  |
| `RESERVE_DAYS` | 3.0 | `services/src/market.rs:170` | A maker keeps this many days of its own use before it sells any. |
| `ASK` | 1.05 | `services/src/market.rs:172` | A maker sells at this over its price, and buys back at this under it. |
| `BID` | 0.85 | `services/src/market.rs:173` |  |
| `CARRIED_PER_HOP` | 0.8 | `services/src/outfitter.rs:20` | The chance a module is carried falls by this a hop from its brand's home… |
| `CARRIED_FAR` | 0.15 | `services/src/outfitter.rs:22` | …to no less than this. |
| `MARKUP_PER_HOP` | 0.05 | `services/src/outfitter.rs:24` | Its price rises by this share a hop (shipping). |
| `SETTLER_CREDITS` | 3000.0 | `sim/src/commerce.rs:16` | What a new settler starts with (credits). |
| `BOARD_EVERY` | 120.0 | `sim/src/commerce.rs:19` | Markets put out their price boards this often (s), over the hypernet. |
| `BOARD_KEPT` | 3.0 * 3600.0 | `sim/src/commerce.rs:21` | Boards kept this long (s): past the slowest way round a system's net. |
| `BUYBACK` | 0.6 | `sim/src/commerce.rs:368` | A module taken out at a refit fetches this share of its price. |
| `REPAIR_PRICE` | 0.3 | `sim/src/commerce.rs:543` | A whole hull's repair costs this share of its frame's price, and takes this share of its frame's mass in metals. |
| `REPAIR_METALS` | 0.05 | `sim/src/commerce.rs:544` |  |
| `INSURANCE_EXCESS` | 0.1 | `sim/src/commerce.rs:546` | A ship lost is replaced, the same hull and fit, for this share of its value. |
| `FARE` | 40.0 | `sim/src/commerce.rs:644` | A passenger's fare (credits): this much, and this much more for each gate on the way (paid on arrival by the settlement fund of the place... |
| `FARE_PER_GATE` | 120.0 | `sim/src/commerce.rs:645` |  |
| `TICK` | 1.0 / 60.0 + 1e-9 | `sim/src/universe.rs:16` | The longest tick (game seconds): pilots act once a tick. |
| `TICK_BUDGET` | 8 | `sim/src/universe.rs:18` | The most ticks a step may take (beyond it, under heavy warp, ticks stretch). |
| `PRESENCE_EVERY` | 6 | `sim/src/universe.rs:21` | Traffic control's look at who's where, every this many ticks. |
| `CORRIDOR_RELEASE` | 1_500.0 | `sim/src/universe.rs:24` | A ship on its final run this close to the station or gate lets the next one start (m). |
| `ENTRY_LEAD` | 60.0 | `sim/src/universe.rs:27` | A ship due out of a gate's tube within this keeps its entrance closed (s): about a final run's length (4 km at 100 m/s), so no one is on ... |
| `STARTING_CREDITS` | 1_000_000.0 | `sim/src/universe.rs:32` | What a new pilot starts with (credits). (For now, while ships are being built and tried: enough to buy any. The economy's balance pass se... |

### News (→ a news outlet record)

| Constant | Value | Where | What it is |
|---|---|---|---|
| `DIGEST_EVERY` | 600.0 | `sim/src/newsroom.rs:19` | An outlet puts out a digest this often (s of world time). |
| `KEPT` | 40 | `sim/src/newsroom.rs:21` | Digests kept. |
| `LISTEN_EVERY` | 5.0 | `sim/src/newsroom.rs:23` | How often outlets listen (s). |

## Two to decide first (with the user)

- **Landing:** `LAND_SPEED` (30 m/s, "touching slower than this lands") is a rule of the old
  game. Your MC-07 has a sourced design landing speed (3.05 m/s) and a failing one (about 8.2). The
  rule belongs to each hull's record (`design.landing_speed`), not to the port. That changes how
  ships land, so it's the user's call, and the ships side's.
- **The person:** walking speed, eye height, jump and reach (`crew.rs`) are a human's. A `person`
  (or species) record would hold them, if the user wants people described in the registry.
