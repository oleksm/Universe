# Food: what feeds the system

*Registry session, 2026-10-04. The user asked for the whole of it: crops, livestock, fruit,
vegetables, the works that process them, the companies, the land. What is in the registry now, the
sum that sizes it, and what waits. Every figure here is a typical figure of Earth's farming, from
memory, marked to review in its record: none was looked up.*

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
- **18 industrial modules** (`SFO/metadata/modules/`) with 76 recipes:
  - growing: field (100 ha, 19 crops), market garden (10 ha, 12), orchard (100 ha, 11), grow hall (1 ha under lamps, 8)
  - animals: livestock barn (pigs, chickens, eggs, milk, cattle, sheep), fish farm
  - works: feed mill, flour mill, oil press, sugar works, dairy, meat works, bakery, brewery, winery, fertiliser works
  - storage: grain silo, cold store
- **9 companies** (`MakerHouse/metadata/makers/`): Eikir Growers, Nacaubun Cold Country Farms,
  Verdance Orchards, Halloran Mills, Brightwater Dairy, Marrow & Pike, Lantern Halls, Greenfall
  Agrochemical, Cellar & Vat. Invented, each with its trade; homes at Port Eikir, Port Nacaubun
  and the station.

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

## The sum: what it takes to feed everyone

A diet of 48 of these foods adding up to 1.53 kg a person a day (bread 200 g, potatoes 200 g, rice
80 g, vegetables 300 g, fruit 200 g, milk 150 g, meat 80 g, and so on), for 220,000 people, grown
in the open and worked back through the recipes:

| Module | How many | Ground |
|---|---|---|
| Field | 295 | 295 km2 |
| Orchard | 39 | 39 km2 |
| Market garden | 131 | 13 km2 |
| Livestock barn | 60 | 0.6 km2 |
| Fish farm | 11 | 0.2 km2 |
| Mills, presses, dairy, meat works, bakery, brewery, winery | about one of each | under 0.1 km2 |
| **All of it** | | **348 km2** |

- Nearly half the fields feed animals: hay 85 km2, maize 52 km2, and barley and oilseed meal besides.
  Without meat, milk and eggs it would be about 190 km2.
- It draws 12 MW. Farming under a sky costs ground, not power.
- It takes in about 9 t of nitrogen and 2 t of hydrogen a day as fertiliser, and gives off 1,050 t
  of manure, 210 t of straw and 580 t of oxygen.
- The same diet under lamps: a grow hall makes 5 to 30 times a field's crop from its hectare and
  draws 3 MW doing it. Feeding 100,000 people that way is 1 to 2 GW.

**Not allowed for:** Treistun e gets two thirds of Earth's light and averages 264 K; d gets 1.8
times and averages 322 K. Yields there will not be Earth's. The recipes are Earth's until someone
works out what those worlds do to a crop.

## What waits on the engine

Placing any of this on the ground needs two values the engine's types do not have, and a type that
refuses what it does not know:

- a zone `use` of `agricultural` (it has port, industrial, commercial, civic, residential);
- a facility `kind` of `farm` (it has foundry, mill, yard, power, warehouse), or one for each of
  farm, food works and store.

With those: at Port Eikir and Port Nacaubun an agricultural zone of about 20 km by 20 km beyond
the port, farm roads, lots, and the farms, works and stores on them as facilities with lines; and
a grow hall at each airless settlement and the station for the fresh food that does not ship.

A module and a good have no maker in the engine's types either, so the companies own nothing
yet: they will own the lots.

## What the user has still to say

- How the food is shared between d and e, and how much the airless settlements grow for
  themselves under lamps.
- Whether the yields are to be worked out for d's and e's light and warmth, or taken as Earth's.
