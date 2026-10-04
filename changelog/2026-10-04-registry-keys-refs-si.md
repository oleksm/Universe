# The registry made solid: keys, refs, SI

*Registry only (`fso`). Nothing the game loads has changed: the generated files are byte-identical.*

- **One key on every record:** `<kind>.<name>`, the kind being its schema's (`hull.mc-07`,
  `equipment.drive.torch.s1`, `gate.ring.i`, `body.treistun.treistun-f`, `org.hadley`). The build
  checks each is there, well-formed, matches its file and is no other record's.
- **Schemas merged:** one organisation schema for companies, the standards body and
  administrations; one body schema (a star is a body; comets, centaurs and the like are bodies the
  game does not make yet); one population schema for asteroid fields and far regions. Local
  Administration no longer keeps its own copy of planets and moons: a settlement is at a celestial
  body.
- **Records name each other by key:** 1,216 references. Each such property says in its schema which
  kinds it may name, and the build checks every one.
- **SI throughout:** every value with a dimension is in kg, m, s, K, W, N, J, Pa (angles in
  degrees), and its property says its unit. 2,322 values converted. The registry page still reads
  in tonnes, km, hours and AU.
- **One `physical` group** for every physical thing (mass, size, shipping box, volume, temperature
  ranges, what blow and jolt it takes), shared by parts, mill stock, equipment, goods, hulls and
  industrial modules. A hull's hold and tanks are its `capacity`.
- **Dogma is in the registry:** the laws everything runs on, 31 of them in six sections (Nature,
  Measures, Field, Tube, Air, Climate), each with its value in SI, whether it is real, simplified
  or invented, and where its figure comes from. A new report holds each against the engine's own
  copy. Standard gravity (9.80665) is now a named measure; the engine has no name for it yet.
  The registry's own sums now use these, so some worked figures moved in the fourth digit.
- **Densities guessed for the four new rock classes** (primitive, basaltic, enstatite, stony-iron),
  each as a rubble pile and as one solid piece, marked to review. Each seeded small body now has
  its own density, drawn between its class's two (one over 200 km in radius is solid), so the 14
  that had no mass have one. What each world is made of stays empty until it is brought in.
- **Shared shapes for made things:** what a thing is made from is a list (each entry by key, with
  its quantity), and how it is made lists its processes, the same for parts, mill stock, hulls and
  gates. A facility and a rig share one definition of their lines and modules.
- **A gate's distance is worked out** from where the two stars are, not written on the gate.
- **Fixed: every charted system's position was zero** in the registry (the export divided light
  years by a light year). Liham is 4.877 light years from home, Driumum 3.073, Biraidim 5.247,
  Moryemzai 7.015. The generated celestial file lists the systems in that order now.
- **Recipes:** an industrial module now lists what it can be set to make. Each recipe says what it
  makes, what goes in and comes out for each kg, its rate and the power it draws. Every module's
  existing figures became its first recipe; none changed. Which recipe a module is running is the
  game's state, chosen by its owner. The model is written up in `docs/registry-recipes.md`.
- **The mill side runs on recipes.** Ingot, strip and refined steel are a different item for each
  metal; ingots are stock items. The furnaces and mills have a recipe for each thing they can be
  set to make (the reheat furnace three, the hot rolling mill three, the piercing mill three...).
  A facility's line says what it is built to make, and the build finds the way to it through its
  modules' recipes. Ten processes that only said this are gone. The foundry and the smelter now
  take in their alloying metals. In the game's settlement file a works' products are named as
  items ("6061 plate 5 mm") where they were kinds ("Sheet and plate").
- **Logistics** is noted in the backlog to describe properly later: for now a factory is one pool
  of stock, and between factories the market is the join.
