# Geology and the registry: a note for the planet simulation

*From the registry session, 2026-10-05, in answer to the brief "World Geology v0" of the same day.
The user's word on the split: "Good split." Proposals to change anything the registry holds about
geology go in this file.*

## The split

**The simulation owns** where each deposit is and how much it holds: its place, tonnage, grades,
depth, geometry, host rock, district and belt. Thousands to a world: world data, like the engine's
belt rocks.

**The registry holds the shared words,** so the simulation, the game and the works that smelt the
ore mean the same thing by each:

| What | Where | Today |
|---|---|---|
| Deposit types | `standards/Celestial/metadata/deposit-types/` | your 14, each with what forms it, what it carries and a typical grade, a middling size, how it clusters, what a survey sees of it |
| Rock units | `standards/Celestial/metadata/rock-units/` | your 10, each with density, magnetic susceptibility, erodibility and what forms it |
| Events and survey methods | `standards/dictionary.schema.yaml` (`geological_event`, `survey_method`) | 9 and 6 |
| Commodities | each deposit type's `carries` | your 15 names mapped to the registry's elements and goods |
| Ore to metal | modules with recipes | a mine, a concentrator, and works for copper, zinc, nickel, chromium, tin, gold and titanium |

All of it is `draft` (a record's `revision`): build against the words, not the figures. The
registry is in SI: a grade is kg in each kg (0.44% is 0.0044; 0.1 g/t is 1e-07), a size is kg, a
length is m. Your own name and unit for each commodity is kept beside it (`said_as`).

## Your commodities, as the registry has them

| You say | The registry has | Note |
|---|---|---|
| Cu, Mo, Au, Ag, Zn, Pb, Ni, Co, Sn, W, Fe | the element | |
| Cr2O3 | chromium, at 68.4% of your figure | the metal is what recipes take |
| Al2O3 | `good.alumina` | bauxite is `good.bauxite` |
| PGE | platinum | six metals as one: is a split by metal possible? |
| diamonds | `good.diamonds` (new) | a carat is 0.2 g |

## What the registry needs that the brief does not have

In the order it matters to what is already described:

1. **Titanium.** It is in nearly every piece of ship equipment. Not in the brief, nor in its list
   of what is missing. Ilmenite and rutile: beach sands, and anorthosite.
2. **Manganese, vanadium, magnesium, sulphur, silicon.** All are in recipes today as bare elements.
   Not in the brief either.
3. **Bulk rock as a resource.** Limestone, clay, quartz sand and aggregate are not deposits: the
   whole unit is the resource. Which rock unit yields which? The registry has put limestone on
   `limestone` and nothing else: is clay from `sandstone-shale`? Is glass sand a unit of its own?
4. **Salt, potash and phosphate.** In your missing list. They are the base of the chemicals and
   the two fertilisers the registry lacks: for us they come before coal and oil.
5. **Worlds that are not Earth-like.** Most of our ports stand on airless and frozen worlds, and
   ships mine asteroids. What does a world with no plates, no seas or no air get? The registry's
   goods already say three things: found on asteroids; on rocky planets and moons; only on
   temperate planets (bauxite, limestone, clay, glass sand, salt). Does that hold?
6. **Asteroids.** The registry has eight rock classes (`Celestial/metadata/rock-classes/`) with
   compositions. Your worlds' ore and those should not contradict each other.
7. **Uranium,** for the fission fuel the registry lists.

## Asked of the output

- **`district_id` and `belt_id`:** agreed, first.
- **A stable id for each deposit,** the same from one bake to the next, so a claim or a mine can
  point at one.
- **Your own ids for the 14 kinds and the 10 units** (`porphyry`, `arc_volcanic`...): the registry's
  records will carry them, so nobody maps by name.
- **A cap on size for each type.** The example's 12 billion tonnes would bend any economy.
- **Which deposit type geochemistry does not answer to** ("almost every"): the registry has put it
  on all 14.
- **A per-type shape,** if there is one: the brief gives shape for each deposit only.

## Open, the user's (2026-10-05)

"We will later need to figure out how to fit geological data for our bodies and where it belongs."
Not settled, and not started. The questions as the registry sees them:

- A body's record (`Celestial/metadata/systems/<system>/bodies/`) is one small file. A world's
  geology is thousands of deposits and a map. Does a body point at a file of the simulation's, or
  hold a summary (how much of each commodity, how many districts), or both?
- Bodies are `seeded`, `curated` or `frozen`. A baked world would be the fourth thing: made by a
  simulation of its history.
- The registry's bodies were seeded by other rules than the simulation's. Which worlds are baked,
  and what do the others say?

## Ore to metal, as the registry now has it (2026-10-05)

- **An ore for each deposit type,** as a good made of what its type carries: 12 new, with iron ore
  and bauxite already there. Ilmenite is the registry's own, on rocky planets and moons, until you
  have a type for it.
- **A mine:** set to the ore that lies where it stands. Its ore and the barren rock over it are
  taken from the place (`from: place`), 0.83 t of rock a tonne of ore.
- **A concentrator:** copper, zinc, nickel and tin concentrates, each from the ores that carry it.
- **Works:** copper smelter, zinc works, nickel smelter, ferroalloy furnace (chromium), tin smelter,
  gold works, titanium works. Each balances by mass; the sulphur of the sulphide ores is caught.

**Won beside the main metal (2026-10-05, later):** molybdenum (off a porphyry, to a molybdenum
works), lead (beside zinc, to a lead smelter, which also gives silver), copper (beside zinc and
nickel), tungsten (beside tin, to a tungsten works), gold and silver (from the copper smelter's
tank slime), the platinum metals (from the nickel smelter, as platinum), diamonds (from kimberlite,
in the concentrator), and laterite nickel (in the ferroalloy furnace, as ferronickel). **Still
lost:** cobalt, and the silver of epithermal and massive sulphide ores.

## After the brief's v0.1 (2026-10-05, later)

**Taken in:**

- **Bulk rock:** each rock unit's record now says what it yields (`yields`, with your quality
  range as you give it) where the registry has the good: aggregate, limestone, glass sand, clay.
  What it has no good for yet is listed beside (`also`): dolomite, dimension stone, pozzolan,
  feldspar, basalt fibre stock, slate, olivine, serpentine. The mine can now be set to limestone,
  clay and glass sand as well as the ores.
- **IDs:** noted (`E4-PCU-010264A-01`: world, type, district, number; `district_id`, `belt_id`).
  Nothing in the registry points at a deposit yet.
- **Caps and the schedule:** noted. Steps 3 to 5 are what the registry waits on most.

**Still asked:**

- Your short codes for the 14 types (`PCU` is porphyry copper) and your ids for the 10 rock units
  (`arc_volcanic`), to carry on the registry's records.
- Which deposit type geochemistry does not answer to.
- When layered intrusions arrive (step 3): the platinum metals split by metal, if it can be had.

## After the brief's v0.3 and the note on airless worlds (2026-10-05, evening)

**Taken in from v0.3:**

- **All 27 deposit types** are records, with their new counts, sizes and grades (the sulphur an
  ore carries is now among them). Three more events (`coast`, `salt flat`, `ocean floor`) and one
  more method (`sonar`) are in the dictionary. A type under the sea says `offshore: true`.
- **Your new commodities, as the registry has them:** an oxide's grade is put as its element's
  (TiO2, V2O5, MgO, ZrO2, K2O, P2O5, Nb2O5, as Cr2O3 before); NaCl is `good.salt`; the rare-earth
  oxides are put as cerium, the commonest of them, which is a stand-in: say if you can split them.
- **An ore for each new type** (12 new goods; rock salt is `good.salt`), and the mine digs them.
- **Won from them so far:** ilmenite from beach sand and potash from potash ore (in the
  concentrator); sulphur from both sulphur ores (a sulphur works); manganese (the ferroalloy
  furnace, as ferromanganese); magnesium from magnesite (a magnesium works); phosphate fertiliser
  from phosphate rock and sulphuric acid, leaving gypsum (a phosphate works).
- **Described but not yet won:** the reef's chromium and platinum, titanomagnetite's vanadium and
  titanium, zircon, the nodules, carbonatite's rare earths and niobium, lithium from brine.

### The five questions of the note on airless worlds

1. **An olivine class: yes, added.** `rock-class.olivine` (A-type), with a density and a colour so
   it can be drawn, and nothing else: no shares by zone, since they should come out of what your
   model breaks. Fill `found` from your runs and I will check it against the falls.
2. **Composition fields: yes, added** to the rock class's `composition`, each an optional range,
   lean to rich: `sulphide`, `titanium`, `kreep` (potassium, the rare earths, phosphorus, thorium
   and uranium together), `helium_3`. No values yet: give me ranges with where they come from and
   I will write them, or propose them here.
3. **A moon's surface units: records, of the kind the ten are.** Anorthosite highland, mare
   basalt, KREEP terrain, ice shell, salt deposits would be `rock-unit` records, each with its
   density, what makes it (new events in the dictionary: a magma ocean, a lava plain, an impact,
   an ice shell) and what it yields. Where each lies on a body stays yours. Send the list when it
   is stable and I will write them as drafts, as the ten were.
4. **Where the ports are: the user's to say.** The facts today, in Treistun: of ten ports without
   air, three stand on airless rocky planets (b, c, f: 562, 421 and 189 K), three on the cratered
   moons of rocky planets (b I, c I, d I), two on cold moons of giants (h II at 112 K, i II at
   94 K); and ships mine asteroids, which is the only mining the game has. So by what is there:
   **airless rocky worlds and moons first, asteroids second, icy moons third.**
5. **Each asteroid its own richness: yes.** The game already gives every rock its own composition
   within its class's range, and the registry's `rich_above` and `rich_yields` turn on it. Use the
   class's ranges as the limits.

**One thing your model should know of the registry's side.** A rock class's `found` shares and the
falls are the registry's present truth for what a belt holds, and the engine seeds belts by them
(`seeding.asteroids`). If your broken bodies give other shares, that is a finding, not an error:
bring it and the records change.
