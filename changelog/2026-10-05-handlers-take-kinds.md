# Handlers take a kind's figures whole; required marks flipped

- **The generator:** each kind of a tagged choice (`equipment.function`'s power plant, drive, gun…)
  is now a struct of its own, which its variant holds and its handler method takes
  (`fn gun(&mut self, it: &EquipmentFunctionGun)`). A figure added to a kind in the registry no
  longer breaks the engine's build: the handler reads it when it's ready to. (The Scientist found a
  new optional figure on a kind failed the build; this was why.)
- **Required, as the Scientist checked every record holds them:** every figure of every equipment
  kind that has figures; a hull's slots, thrusters and flight; a module's physical; a recipe's rate
  and power; an amount's item and quantity. The engine reads them as plain values, not options.
- **Inputs drawn where a module stands** (`from: place`, the world's air, rain or ground water, for
  open-sky modules) don't come out of a works' store, aren't bought, count as no supply of a line,
  and cost nothing in a worked-out price.
