# Recipes: how a thing is made

*A proposal from the registry side, 2026-10-04. Nothing is migrated yet. For the user and the
integrator to agree before any record moves.*

## The miss

A line can be set to make more than one thing. Nothing in the registry says what it is set to
make, what that takes, or what it can be set to at all. Today the answer is spread over three
places, none of them complete:

- a **module** carries one fixed set of inputs and outputs (`inputs.materials`, `rate.product`),
  as if a machine made one thing;
- a **process** carries figures per tonne for a method (`inputs`, `outputs`, `energy`);
- an **item** says what it is made from and by which processes (`made_from`, `making`).

## One model, for everything made

Four records, the same for a tonne of alumina, a plate, a hull part or a gate ring. No split by
industry.

| Record | What it is | What it holds |
|---|---|---|
| **Item** | A thing: a good, a stock item, a part, a hull, a piece of equipment | What it is. Not how it is made. |
| **Recipe** | One way of making one item | `makes`, `inputs`, `steps`, `outputs`, `changeover` |
| **Module** | A machine | Its limits (size, most it can push through, power) and its **range**: what it can be set to |
| **Process** | A method, as knowledge | The science: what happens, at what temperature, with its sources. No quantities for a plant. |

And one thing that is **not** in the registry:

| Game state | What it is |
|---|---|
| **Campaign** | Which recipe a line is running now. Its owner decides. Switching is a changeover. |

### A recipe

```yaml
identity:
  key: recipe.al6061-plate
  name: "6061 plate, rolled"
makes: stock.al6061-pl-5          # one item; or a family (see "Open" below)
inputs:                           # per kg of what it makes, all steps together
  - { item: stock.al6061-ingot, quantity: 1.524 }
steps:                            # in order
  - { process: process.rolling, module: module.reheat-furnace,    does: "Heated through to rolling temperature.", rate: 33.3 }
  - { process: process.rolling, module: module.hot-rolling-mill,  does: "Squeezed into a long thick strip.",      rate: 27.8, yield: 0.82 }
  - { process: process.rolling, module: module.cold-rolling-mill, does: "Rolled cold to its final thickness.",    rate: 16.7, yield: 0.84 }
  - { process: process.rolling, module: module.finishing-line,    does: "Levelled, tempered, cut.",               rate: 16.7, yield: 0.95 }
outputs:                          # besides what it makes, per kg of it
  - { item: good.scrap, quantity: 0.524 }
changeover: {}                    # time and loss to switch a line to this recipe: not known yet
```

Every figure in that example is in the registry today (the four modules' throughputs in kg/s and
their inputs per unit of output); it only moves. `changeover` has no figure yet and is left empty.

- **A step** is a process done on a kind of module, with what that step is set to: its rate for
  this item, its yield, and any setting the method needs (temperature, passes).
- **Rate** is the recipe's, capped by the module's maximum. The line's rate is its slowest step.
- **Power** stays the module's.
- **More than one recipe may make the same item**: steel from ore or from scrap, aluminium from
  bauxite or from anorthosite. This is why a recipe is its own record and not a group on the item.

### A module's range

```yaml
range:
  - { form: plate, materials: [material.aluminium-alloy-6061, material.structural-steel-a36], thickness: [0.003, 0.15], width: [0, 4] }
```

What it can be set to make: a form, the materials, the sizes. A line can run a recipe if, for
each step, it has a module of that kind whose range covers the item. No range figures exist
yet; they are to be sourced or guessed, marked for review.

### What decides

The owner of the line, a player or an NPC, by choosing a campaign. The registry says what is
possible (range and recipe) and what switching costs (changeover). It never says what is made.

## What moves

| Today | Becomes |
|---|---|
| `module.inputs`, `module.outputs`, `module.rate.product` | the steps of recipes; the module keeps `rate` as a maximum and gains `range` |
| `process.inputs`, `outputs`, `energy`, `rate` (quantities) | recipes; the process keeps what is known of the method, and its sources |
| `process.equipment.steps` | `recipe.steps` |
| `item.made_from`, `item.making` (parts, mill stock, hulls, gates) | a recipe for that item: `inputs` and `steps` |
| the nine goods that are forms of stock (ingot, bar, tube, sheet-and-plate, parts, hulls, forgings, cut-blanks, formed-panels) | gone: a recipe names the stock item it makes |
| a facility line's `process` and `also` | the recipes the line is built to run |
| `content/base/recipes.ron` (hand-written) | generated from these |

This answers items 6 and 7 of the SSOT request: a module's output is not a good and not one stock
item, it is whatever recipe it is set to; and flows live in one place, the recipe.

## Open

1. **One item or a family.** `makes` one stock item is exact; 6061 plate in five gauges is then
   five recipes that differ in one number. A family (`makes` a material and a form, the size a
   parameter) is fewer. Proposal: start with one item each, since there are nine stock items, and
   add families when the count hurts.
2. **Parts.** 185 parts each have their own input quantity, so each is its own recipe. They can be
   written from `made_from` and `making` as they stand, with no new figures.
3. **Who owns a recipe.** A recipe could be a maker's product, published and licensed, as a
   blueprint is. Proposal: allow `identity.maker`, leave it empty for now.
4. **Order of work.** The MC-07's chain first (rock to hull), since its figures are all there,
   and the chain report is the check that nothing was lost.
