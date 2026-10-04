# Recipes: how a thing is made

*The production model the user settled on 2026-10-04, and what has been done to the records since
(see "Done so far"). It began as a proposal; the sections below are in the order it was decided.*

## The miss

A line can be set to make more than one thing. Nothing in the registry says what it is set to
make, what that takes, or what it can be set to at all. Today the answer is spread over three
places, none of them complete:

- a **module** carries one fixed set of inputs and outputs (`inputs.materials`, `rate.product`),
  as if a machine made one thing;
- a **process** carries figures per tonne for a method (`inputs`, `outputs`, `energy`);
- an **item** says what it is made from and by which processes (`made_from`, `making`).

## One model, for everything made

*Revised after the user's correction: a recipe belongs to the module, and a setup is what runs.*

The same for a tonne of alumina, a plate, a hull part or a gate ring. No split by industry.

| Record | What it is | What it holds |
|---|---|---|
| **Item** | A thing: a good, a stock item, a part, a hull, a piece of equipment | What it is. Not how it is made. |
| **Module** | A machine: one step of a line | Its limits (size, power, the most it can push through) and its **recipes**: everything it can be set to make |
| **Recipe** | One thing a module can be set to make. It is the module's, written in the module's record. | `makes`, `inputs`, `outputs`, `rate`, `power`, `changeover` |
| **Process** | A method, as knowledge | The science: what happens, at what temperature, with its sources. No quantities for a plant. |

And what runs, which is **not** in the registry:

| Game state | What it is |
|---|---|
| **Setup** | A module set to one of its recipes. It takes that recipe's inputs and gives that recipe's item. Its owner chooses it; changing it is a changeover. |

A module can make several things; at any moment its setup says which one.

### A module with its recipes

```yaml
identity:
  key: module.cold-rolling-mill
  name: Cold rolling mill
physical: { length: 150, width: 40, height: 15 }
recipes:
  - makes: good.cold-rolled-strip         # today's one fixed output, as it stands
    process: process.rolling
    inputs:  [{ item: good.hot-rolled-strip, quantity: 1.19 }]     # per kg of what it makes
    outputs: [{ item: good.scrap, quantity: 0.19 }]
    rate: 16.7                            # kg/s, set to this
    power: 38000000                       # W, set to this
    changeover: {}                        # time and loss to set it to this: not known yet
  # a second recipe is a second thing it can be set to: another metal, another gauge
```

Every figure there is the module's own today (`rate.throughput`, `inputs.materials`,
`outputs.by_products`, `needs.power`); it only moves under a recipe. A module that has one recipe
today has one after; more are added as they are known.

- **A line** (a facility's, a rig's) is modules in order. Each has its own setup. What one gives is
  what the next takes: a line makes plate when each of its modules is set to the recipe that feeds
  the next.
- **What a module can make** is its list of recipes. There is no separate `range`.
- **More than one module may make the same item**, by different recipes: steel from ore in one,
  from scrap in another.

### What a line's `makes` is

A facility's line says what it is built to make. That is the seeded world's starting setup, not a
limit: the game loads it as an ordinary setup, and the owner can set the modules to any other of
their recipes. The rules here for markets, full stores and no transport inside a works are the
game's mechanics; this file is their spec.

### What decides

The owner of the module, a player or an NPC, by choosing its setup. The registry says what each
module can be set to and what switching costs. It never says what is made. The seeded world's
starting setups are seed state, written with the facility that has the module.

## What moves

| Today | Becomes |
|---|---|
| `module.inputs`, `module.outputs`, `module.rate`, `module.needs.power` | the module's first recipe |
| `process.inputs`, `outputs`, `energy`, `rate` (quantities) | kept only where a module's recipe does not already say it, then moved to the module that does the step; the process keeps what is known of the method, and its sources |
| `process.equipment.steps` | which modules, in order, a line needs: stays with the process as its outline |
| `item.made_from`, `item.making` (parts, mill stock, hulls, gates) | a recipe on the module that makes it (a plate: the finishing line; a part: the shop's module; a hull: the building dock) |
| the nine goods that are forms of stock (ingot, bar, tube, sheet-and-plate, parts, hulls, forgings, cut-blanks, formed-panels) | gone where a recipe names the stock item itself; the ones that pass between modules (hot ingot, strip) stay as items |
| a facility line's `process` and `also` | the line's modules; what each is set to is its setup |
| `content/base/recipes.ron` (hand-written) | generated from the modules' recipes |

This answers items 6 and 7 of the SSOT request: a module's output is whatever recipe it is set
to, and flows live in one place, the module's recipes.

## Decided (user, 2026-10-04)

- A recipe belongs to the module. A setup (game state) is a module set to one of its recipes.
- How many recipes a module has is the module's own business, as in life. A module that can make
  only one thing has one recipe, and it needs no choosing.
- What passes between modules is a different item for each metal: hot strip of steel and hot strip
  of aluminium are two things.
- The process record is to go: what it holds moves to recipes. Those with no module yet stay parked
  until their machine is described.

- *(Coarse first: stock moves inside a works at once and at no cost of energy. That is a
  simplification, to be replaced when logistics is described: backlog item 11.)*
- **Inside one factory there is no transport.** A facility (or a rig) is one pool of stock: what a
  module puts out is there for any other module of the same facility to take in. No links,
  conveyors or routes between its modules, and its modules need no identities of their own for
  this. Moving stock is a matter between facilities, and that is not designed yet.

- **Between factories, the market.** What a factory makes and does not use itself goes on the
  market of the settlement it stands in, and what it needs and does not make it buys there. The
  registry describes no route from one factory to another: the market is the join, and prices and
  what is on offer are the game's state. Hauling between settlements is ships, as now.

- **Everything physical is somewhere, and can be seen.** No item is only a number in a ledger. What
  a factory holds lies in its storing module. What is on the market lies in a warehouse an
  exchange has approved (`facility.exchange`, as Trethi Warehouse is): selling moves it there, or
  it is sold where it lies. Anyone who goes there can see it.

- **When storage is full, what fills it stops.** A module can run only while its factory's storing
  modules have room for what it makes. Full yards stop the modules that feed them until some stock
  is taken away (used by another module, sold, hauled off). This is the game's to run; the registry
  gives the room (`capacity.holds`, `capacity.volume`) and, as a measure, how long each works'
  room lasts flat out with nothing taken away.

- **Between modules, through the store** (the user, later the same day, correcting step 3): what a
  module makes goes to the works' storing module, and the next module takes it from there. So a
  step's output is stock, with a record, lying in the yard between steps. The cutting table and
  the panel former are steps again.

## Done so far

**Step 1 (2026-10-04): every module's own figures are its first recipe.** `module.recipes[]`:
`makes` (or `supplies`, W, for one that makes power), `inputs` and `outputs` (`item`, `quantity`
per kg of what it makes), `rate` (kg/s), `batch`, `power` (W drawn), `changeover` (empty: no
figures yet). What a store or a dock can hold and move is `capacity`. `rate`, `inputs`,
`outputs` and `needs.power` on a producing module are gone. The power station's fuel was per MWh
and is now kg/s at full output, so the last non-SI figure is gone. 31 modules, no figure changed.

**Step 2 (2026-10-04): the mill side runs on recipes.**

- *A different item for each metal.* New: three ingots as stock items (`stock.al6061-ingot`,
  `stock.st4340-ingot`, `stock.sta36-ingot`); goods `hot-ingot-6061/-4340/-a36`,
  `hot-rolled-strip-6061`, `cold-rolled-strip-6061`, `refined-steel-4340/-a36`. Gone: the goods
  `ingot`, `hot-ingot`, `hot-rolled-strip`, `cold-rolled-strip`, `sheet-and-plate`, `bar`, `tube`,
  `forgings`, `refined-steel`.
- *Recipes for each.* The reheat furnace has three (one a metal), the hot rolling mill three
  (aluminium strip, 4340 plate, A36 plate), the ladle station and the casting bay two (the two
  steel grades), the finishing line two (plate, sheet), the piercing mill three (the tube sizes),
  the cutting table three (the stock it cuts). A recipe has `does`: what happens in it.
- *A mill stock item has no `making`.* What makes it is the recipe that names it.
- *Ten processes are gone:* the two alloyings, the two steelmakings, rolling, the two plate
  rollings, bar rolling, tube making, forging. Their steps' words are the recipes' `does`. The
  alloying additions they listed are now inputs of the casthouse's and the ladle station's
  recipes, so those metals are bought, not from nowhere.
- *A line says what it makes:* `makes: stock.al6061-pl-5`, `also: [stock.al6061-sh-2]`, and its
  modules. The build finds the route back through its modules' recipes and refuses a line whose
  modules do not lead to it.
- *Figures used again are marked.* Where a module had one set of figures and now has a recipe for
  each metal, gauge or size, the others are that set again, marked to review (49 figures).
- *Check:* the MC-07's chain and the ring's are complete as before (56 of 56 parts); every
  recipe balances; the facilities' most is the same but for the foundry and the smelter, which
  now count the alloying metals they take in (about 2% more ingot).

**What the game loads.** `settlements.ron`: the same shape; a facility's `makes`, `takes` and
`gives` now name the item (`"6061 plate 5 mm"`, `"A36 steel ingot"`) where they said `"Sheet and
plate"` and `"Ingot"`, and the foundry's and smelter's `takes` list the alloying metals. Nothing
else changed. The world's tests pass with it.

**Step 3 (2026-10-04): shop work, scrap, storage.** The user: "assume preparation by factory
module", "scrap as its own stock output", "a storage module on the factory so all the stock is
kept in it".

- *A part is made in one module.* `making.module` on a part, a hull and a gate names it (plate
  parts: the welding bay; machined parts: the machining centre; assemblies: the assembly shop; the
  hull: the building dock, which also fits it out). That is a recipe of that module's: what goes in
  is the part's own `made_from`, what is cut away comes out as scrap of its material. The module is
  taken to do its own preparation. (Undone in step 5 for plate work: cutting and forming are steps
  of their own again.)
- *The shop modules' own recipe* is by weight: `makes: good.parts` (or `good.hulls`), a rate and a
  power, no inputs. It says how fast, not what.
- *Scrap is a stock item of each metal:* `stock.al6061-scrap`, `stock.st4340-scrap`,
  `stock.sta36-scrap`. The good `scrap` is gone, with `cut-blanks` and `formed-panels`.
- *The last seven processes with modules are gone* (plate work, machining, fitting, fitting out,
  hull assembly, panel making, ring assembly). `making.processes` is gone from every record. A
  line no longer names a process at all: it says what it makes.
- *Storage.* A works that makes things must have a module that stores, or the build refuses it. A
  new module, the stock yard (the ingot yard's figures, to review), stands at the mill and the
  yard, which had none.
- *Check:* both chains complete as before; the MC-07 is built in 21.6 days at the yard, as before.

**What the game loads, since step 2.** `settlements.ron`: the mill and the yard have a stock
yard (a block more each, `holds: 50000.0`); the yard no longer `takes` sheet (what a shop takes is
its parts' own, which depends on what it is set to make) and draws 1.72 MW, not 2.05 (its cutting
table and panel former stand idle). `industry.ron`: the stock yard added.

**Step 4 (2026-10-04): warehouses and zoning at every port.** Each of Treistun's other nine ports
(Aipika, Eikir, Fabindum, Lisaur, Nacaubun, Seiwiti, Sirnendis, Weisonum, Zaudalein) now has a
port zone, an industrial zone with nothing on it yet, Dock Road, a parcel south of the hangar and
a warehouse on it approved by the Treistun Exchange: all as Port Trethi's, invented, and said so
in each record. What lies on the market at a port lies in its warehouse. The station and the
three gates have none: they are not ground, and wait for stations to be described as structures.
`settlements.ron` has nine settlements more (288 lines).

Each of those nine ports also has a power station (one fusion station, on a second lot owned by
Aurel on East Road) and a power line from it to the warehouse; Port Trethi has a fifth line, to
its warehouse, which had none. One fusion station (400 MW) is the smallest there is a record of
and far more than a warehouse's yard draws (0.5 MW): what a port itself draws is not described.
The power report now counts what stands beside a works' lines and draws all the time (a yard, a
store), which it left out.

How long each works' room lasts flat out, nothing taken away: the mill 12 days, the orbital works
26, the smelter 46, the foundry 66, the yard 482.

**Step 5 (2026-10-04): plate work in three steps again, through the store.** The cutting table
has a recipe for each stock it cuts (stock in, cut blanks out, scrap of that metal), the panel
former one for each (blanks in, formed panels out), and the welding bay makes the part from the
panels of its stock. Six stock items more: cut blanks and formed panels of 6061 sheet 2 mm, 6061
plate 5 mm and A36 plate 20 mm (forms `blank` and `panel`). The yard has a line that makes
panels (cutting table, panel former) and a line that makes parts (two welding bays); the panels
lie in its stock yard between. It takes in sheet again (2.3 t/h) and draws 2.05 MW. The cutting
table's sourced figures are back on its recipes. A plate part's chain has a step more: a yard is
built to cut and form its stock.

**Step 6 (2026-10-04): nothing is made of nothing.** The user: "Recipe has to have input,
according to physics nothing appears from nothing."

- A recipe requires `makes` and at least one input: the schema refuses one without.
- The six shop modules (welding bay, machining centre, panel line, assembly shop, assembly jig,
  building dock) had a recipe by weight that made "parts" or "hulls" out of nothing, or out of
  "parts". They have none now. Each has a `throughput` (rate, power): how much it can put through,
  whatever it is given. Its recipes are the parts, hulls and gates that name it in
  `making.module`, and what goes into each is its `made_from`, or its own parts.
- The build refuses a part that is made in a module with nothing going in. None is.
- The goods `parts` and `hulls` are gone: the last of the names for stock by the tonne.
- A line of shop modules has no `makes`: it makes whatever parts name its modules.
- The yard no longer shows `takes` of "Parts" in `settlements.ron`: those were its own parts
  passing from one of its lines to the next.

## Next

4. The 26 processes with no module: parked as they are until their machines are described.
5. What a shop takes in at most, for the game: today nothing says it, since it depends on the
   parts it is set to make.

## Open

1. **How many recipes a module carries.** A recipe for each stock item is exact; a finishing line
   that cuts plate in five gauges then has five recipes that differ in one number. Proposal: one
   recipe for each item for now (there are nine stock items), and a recipe that takes a size as a
   parameter when the count hurts.
2. **Parts.** 185 parts, each with its own input quantity: 185 recipes on the shop modules that
   make them (plate work, machining). They can be written from `made_from` and `making` as they
   stand, with no new figures. Kept in files beside the module, as parts are kept beside a hull,
   so a module's record stays readable.
3. **What passes between modules.** Hot ingot and strip are of one metal or another. Either an
   item for each metal, or one item with the metal carried along. Proposal: decide when a second
   metal goes down the same line.
4. **Order of work.** The MC-07's chain first (rock to hull), since its figures are all there,
   and the chain report is the check that nothing was lost.
