# Food: what feeds the system

*Registry session, 2026-10-04. The user asked for the whole of it: crops, livestock, fruit,
vegetables, the works that process them, the companies, the land. What is in the registry now, the
sum that sizes it, and what waits. The figures were first put in from memory and then looked up:
`standards/sources/research_food_crops.json`, `research_food_livestock.json` and
`research_food_light.json` hold each figure with its page and the line it was read from. What is
still a guess is marked to review in its record.*

## Who is fed

220,000 people in Treistun as the game seeds it: two worlds with air at 60,000 each (Port Eikir on
d, Port Nacaubun on e), three airless planets' ports at 20,000, the station at 25,000, five moon
outposts at 3,000. A person uses 1.5 kg of food a day (the game's figure, and a fair one for fresh
food): 330 t a day.

## What is in the registry

- **75 goods** (`SFO/metadata/goods/`), 57 of them food, each traded as food:
  - grains and roots: wheat, rice, maize, barley, oats, potatoes, sweet potatoes, cassava
  - pulses and oilseeds: soybeans, beans, peas, lentils, chickpeas, rapeseed, sunflower seed, groundnuts
  - sugar crops: sugar beet, sugarcane
  - vegetables: tomatoes, onions, cabbage, carrots, lettuce, cucumbers, peppers, pumpkins, spinach, garlic
  - fruit and nuts: apples, oranges, lemons, bananas, grapes, mangoes, strawberries, watermelons, olives, almonds
  - coffee, tea, cocoa
  - from animals: milk, eggs, beef, pork, mutton, chicken, fish fillets, cheese, butter
  - made: flour, bread, vegetable oil, olive oil, sugar, beer, wine
  - between steps: hay, animal feed, live cattle, pigs, sheep, chickens and fish
  - given off: straw and stalks, manure, bran, oilseed meal, pomace, beet pulp, bagasse, whey, spent grain, offal and hides
  - brought in: fertiliser
  Each has its bulk density, and what spoils says how cold it must be kept.
- **18 industrial modules** (`SFO/metadata/modules/`) with 75 recipes:
  - growing: field (100 ha, 19 crops), market garden (10 ha, 12), orchard (100 ha, 11), grow hall (1 ha under lamps, 7)
  - animals: livestock barn (pigs, chickens, eggs, milk, cattle, sheep), fish farm
  - works: feed mill, flour mill, oil press, sugar works, dairy, meat works, bakery, brewery, winery, fertiliser works
  - storage: grain silo, cold store
- **9 companies** (`MakerHouse/metadata/makers/`): Eikir Growers, Sunward Planters,
  Verdance Orchards, Halloran Mills, Brightwater Dairy, Marrow & Pike, Lantern Halls, Greenfall
  Agrochemical, Cellar & Vat. Invented, each with its trade; homes at Port Eikir and the station.

## Nothing from nothing

Every recipe balances by mass. A crop is made of carbon dioxide and water, and gives off oxygen:
for each kg of dry plant, 1.47 kg of carbon dioxide and 0.6 kg of water go in and 1.07 kg of
oxygen comes out, with the water the crop holds besides, a little fertiliser, and its straw. An
animal eats feed and hay, drinks, breathes oxygen in and carbon dioxide out, and the rest is
manure. So a farm takes in what a works gives off (the potline's carbon dioxide) and gives off
what people need (oxygen).

**Open for the engine:** on a world with air and rain, a field's carbon dioxide and most of its
water are the world's own, free. In a grow hall they are brought. The recipes list them either
way; nothing in a record says yet that a module stands under an open sky. Water a crop only
passes through (irrigation that goes off as vapour) is not counted.

## What was looked up, and what is still a guess

- **Sourced:** all 42 crops' yields (FAOSTAT world averages for 2024; hay is the United States');
  their dry share; storage temperatures; feed for each kg of pig, chicken, egg, beef, lamb, fish
  and milk; water a head; eggs a hen; milk a cow; carcass and meat shares; flour, oil, beet sugar,
  cheese, butter, bread, malt and wine yields; yields under lamps for lettuce, tomatoes, peppers,
  potatoes, wheat and soybeans; the efficacy of lamps.
- **Still a guess:** how much straw 26 of the crops leave; bulk densities of roots, vegetables and
  fruit; made tea (only fresh leaf is reported); olive oil from olives; sugar got out of cane;
  spinach under lamps; every module's size and what its machines draw; every works' rate; the
  herd and flock a barn holds.
- **Moved when looked up:** spinach yields twice what was guessed, beans three quarters; a hard
  cheese takes 14.4 kg of milk, not 10; cattle need 6.5 kg of feed for each kg, not 8; fish are
  42% fillet, not half.

## The sum: what it takes to feed everyone

A diet of 48 of these foods adding up to 1.53 kg a person a day (bread 200 g, potatoes 200 g, rice
80 g, vegetables 300 g, fruit 200 g, milk 150 g, meat 80 g, and so on), for 220,000 people, worked
back through the recipes at Earth's yields:

| Module | How many | Ground |
|---|---|---|
| Field | 284 | 284 km2 |
| Orchard | 40 | 40 km2 |
| Market garden | 88 | 9 km2 |
| Livestock barn | 57 | 0.6 km2 |
| Fish farm | 13 | 0.3 km2 |
| Mills, presses, dairy, meat works, bakery, brewery, winery, fertiliser works | one of each | under 0.1 km2 |
| **All of it** | | **334 km2** |

- Nearly half the fields feed animals. It draws about 12 MW: farming under a sky costs ground,
  not power.
- **Under lamps:** a kilogram of lettuce costs about 10 kWh, which is what vertical farms report
  (10 to 20). A kilogram of wheat costs about 830 kWh. Leaf under lamps is dear; grain under lamps
  is absurd. NASA's own figure for a whole diet is 20 to 50 m2 of beds a person.

## What Treistun's two worlds with air do to a crop

Worked out, not measured, from sourced pieces; the engine's to confirm.

**Light.** Treistun is a K star at about 5,000 K (from its luminosity and radius in the
registry). Less of a cooler star's light is the kind plants use: about 30% against the Sun's 37%
(a black body's shares; no measured K-star figure was found). Cress grown under a simulated
K-star's light grew as under the Sun's.

| | Sunlight at its orbit | Light plants can use, against Earth's |
|---|---|---|
| Treistun d | 2,410 W/m2 (1.77 of Earth's) | 1.43 |
| Treistun e | 916 W/m2 (0.67) | 0.54 |

**Warmth.** By the game's own climate rule (`climate.rs`: warm at the equator, a quarter cooler in
absolute temperature at the poles, 33 K added by Earth-like air), the year's mean by latitude:

| Latitude | Treistun d | Treistun e |
|---|---|---|
| Equator | 48 C | -10 C |
| 25 | 36 C | -20 C |
| 30 | 30 C | -24 C |
| 35 | 25 C | -29 C |
| 40 | 19 C | -33 C |
| 46 (Port Eikir) | 12 C | -39 C |
| 50 | 6 C | -43 C |
| 55 | 0 C | -48 C |

(Port Nacaubun, at 24 north on e, is at -19 C.)

- **Treistun d farms in two bands.** From 38 to 50 degrees it is Earth's temperate country: wheat,
  barley, potatoes, beet, apples (wheat grows from 5 C and likes 15 to 20). From 25 to 35 it is
  hot country: rice, maize, sugarcane, bananas (maize likes 30 to 37 and stops at 45). Nearer the
  equator it is too hot for any of them; past 55 it is frozen. Port Eikir sits in the temperate
  band, which is what its story says it was put there for.
- **Treistun e does not farm in the open at all.** Its warmest ground averages -10 C. Each degree
  over a crop's best costs 3 to 7% of its yield; e is 25 degrees under the least wheat will grow
  at. The rule has no seasons, and e's axis leans 17 degrees, so a summer may thaw its equator:
  that is the engine's to say. Under glass it would need as much heat as a grow hall needs light.
- **Yields on d:** with 1.43 times the usable light, a crop that uses all it is given (maize,
  sugarcane: their leaves are not sated in full sun) should give about 1.4 times Earth's; the rest
  (their leaves sate at a quarter of full sun, though a whole field does not) are taken at 1.2.
  A guess built on sourced pieces, to be reviewed. With it the 334 km2 becomes about 270 km2.

The recipes keep Earth's yields: a world's light and warmth are the world's, not the field's. How
the engine applies them to a module that stands under a sky is open (see below).

## A start (the user: "put something for start, the market will balance later")

A starting state, not a plan anyone is held to.

**What is made in a place goes on that place's market.** A farm or a works sells into the market
of the settlement it stands at; the other settlements buy what ships bring. Nothing in the registry
says who feeds whom.

**Treistun d grows it all**, round Port Eikir and south of it:

| Band | Latitude | Grows | Fields | Orchards | Market gardens |
|---|---|---|---|---|---|
| Temperate (round Port Eikir) | 38 to 50 | wheat, barley, oats, potatoes, rapeseed, sugar beet, peas, beans, hay; cabbage, carrots, onions, garlic, lettuce, spinach; apples, strawberries, grapes | about 170 | about 8 | about 40 |
| Hot | 25 to 35 | rice, maize, soybeans, sunflower, groundnuts, chickpeas, lentils, cassava, sweet potatoes, sugarcane; tomatoes, peppers, cucumbers, pumpkins, watermelons; oranges, lemons, bananas, mangoes, olives, almonds, coffee, tea, cocoa | about 60 | about 25 | about 34 |

With them: 57 livestock barns, 13 fish farms, and one each of the feed mill, flour mill, oil
press, sugar works, dairy, meat works, bakery, brewery, winery and fertiliser works. About 270 km2
at d's yields. The hot band is some 1,500 km south of Port Eikir: it wants a port of its own, or a
road and a railhead. That is a settlement the registry does not have.

**Everywhere else grows its own leaf and salad under lamps** (lettuce, spinach, tomatoes,
peppers, which do not ship) and buys the rest:

| Settlement | Grow halls | Power |
|---|---|---|
| A moon outpost (3,000) | 1 (it needs a quarter of one) | under 1 MW |
| An airless planet's port (20,000) | 2 | 3 MW |
| The station (25,000) | 3 | 4 MW |
| Port Nacaubun on Treistun e (60,000) | 5 | 9 MW |

Port Nacaubun's 60,000 people are fed from Treistun d like everyone else. **The story follows the
physics** (the user, 2026-10-04: "what is possible is possible"): e is a cold world with air, ice
and water, settled first and never able to farm; d's fields feed the system. The records say so
now (the two worlds' stories, the two ports', the food companies').

## The hydroponic hall: food anywhere

The user, 2026-10-04: "we need to grow more food or everyone is going to die... something like
airtight hydroponics... installed on any planet, but the difference to balance temperature will
cost energy, so hopefully the market will price it."

The grow hall is now that: `module.grow-hall`, the **hydroponic hall**. An airtight hall with a
hectare of troughs in tiers under lamps. No sky, no soil, no weather: it stands on any world, in
orbit or underground.

**What it takes in.** Everything a crop is made of, because nothing comes from outside: carbon
dioxide (people breathe it out), water, all its nutrients as fertiliser, a growing medium for the
roots that clogs and is changed, and light. It gives the crop, oxygen, the plant's leavings and the
spent medium.

**What it grows.** 30 crops: every field and garden crop, not only salad. Seven are as measured
under lamps (lettuce, tomatoes, peppers, potatoes, wheat, soybeans; spinach as lettuce). The other
23 are worked out from the light: NASA's chamber crops made 0.2 to 0.6 g of dry crop for each mol
of light, leaf and root at the top, grain at the bottom. Trees it does not grow: fruit, nuts,
olives, coffee, tea and cocoa still need an orchard under a sky.

**What a kilogram costs in power.**

| Crop | From a hectare a year | Power | For each kg |
|---|---|---|---|
| Lettuce, spinach | 800 t | 0.9 MW | 10 kWh |
| Cabbage, cucumbers, carrots, onions | 720 to 1,080 t | 2.9 MW | 23 to 35 kWh |
| Tomatoes | 650 t | 2.3 MW | 32 kWh |
| Potatoes, sugar beet, sweet potatoes | 350 to 450 t | 2.1 to 2.9 MW | 53 to 68 kWh |
| Maize | 58 t | 2.9 MW | 430 kWh |
| Wheat, rice, barley, pulses, oilseeds | 24 to 52 t | 1.7 to 4.9 MW | 630 to 830 kWh |

Leaf and root are dear; grain is ruinous; meat fed on grain from a hall is worse. The market will
say so.

**Feeding everyone from halls alone,** the same diet less what grows on trees:

| | Halls (hectares of beds) | Power | For each person |
|---|---|---|---|
| Without meat, milk or eggs | 940 | 3.2 GW | 43 m2, 15 kW |
| With them | 2,030 | 6.4 GW | 92 m2, 29 kW |

That is eight fusion power stations for the plain diet and sixteen with animals, against 12 MW for
the same food from Treistun d's fields. NASA's own reckoning for a crew is 20 to 50 m2 a person:
the plain diet lands inside it.

**The heat.** Every watt the lamps draw ends as heat in the hall, and has to be got out. Where it
stands decides what that costs: where outside is colder than the 22 C inside, the heat runs out
by itself and fans are all it takes; where outside is hotter, it has to be pumped uphill.

| Where it stands | Outside | Power to shed the heat, on top of the lamps' |
|---|---|---|
| Port Trethi (Treistun f), Port Nacaubun (e), the cold moons, orbit | 80 to 254 K | about 2% |
| Port Eikir (Treistun d, 46 north) | 285 K | about 2% |
| Port Sirnendis (d's moon) | 306 K | 7% |
| Treistun d's equator | 322 K | 18% |
| Port Lisaur (Treistun c) | 421 K | 85% |
| Port Zaudalein (Treistun b) | 562 K | 180% |

Worked out, not in the records: a heat pump at half of what physics allows at best, against the
ground's mean temperature. On an airless world the heat goes out through radiators, and shade or
a hole in the ground would do better than this. **The record cannot say this yet:** a recipe has
one power figure. For the engine to charge by where a hall stands, a module needs to say what
heat it must shed, what temperature it holds, and how well its plant does it (asked below).

**What it is built of.** Eight components, about 3,100 t (`SFO/metadata/parts/grow-hall/`): a
pressure shell of 2,500 t of steel plate, lining, racks and troughs, tanks and pipes, two airlocks
(all cut from stock a yard can make), and a lamp array, a climate plant, and pumps and controls,
which nothing yet says how to make. All guesses, marked.

## What waits on the engine

Placing any of this on the ground needs two values the engine's types do not have, and a type that
refuses what it does not know:

- a zone `use` of `agricultural` (it has port, industrial, commercial, civic, residential);
- a facility `kind` of `farm` (it has foundry, mill, yard, power, warehouse), or one for each of
  farm, food works and store.

With those: at Port Eikir an agricultural zone of about 15 km by 15 km beyond the port, farm
roads, lots, and the farms, works and stores on them as facilities with lines; and grow halls at
every other settlement for the fresh food that does not ship.

A module and a good have no maker in the engine's types either, so the companies own nothing
yet: they will own the lots.
