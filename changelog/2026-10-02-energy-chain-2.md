# The energy chain, part 2: deuterium

- **Ships' fuel is deuterium** (catalysed D–D fusion: `FUSION_ENERGY` 3.45e14 J/kg; liquid at
  `FUSION_FUEL_DENSITY` 163 kg/m³). The goods kind `goods.fuel` is now DEUTERIUM, in grades and forms
  (no more kerosene, hydrazine and helium-3 under one name), at its real mass and stowage.
- **Made where there's water:** the deuterium plant processes local sea or ice (a tonne of deuterium in
  about 31,000 t of water: `DEUTERIUM_IN_WATER`), at farm worlds (Earth-like seas, 6 plants:
  120 t/day) and outposts (ice, 1: 20 t/day). **Stations no longer make fuel:** they want it, and
  traders bring it. A system without seas or ice imports its fuel across the gates.
- **Helium-3 is the premium fuel to come** (gas giant skimmers, when players build them): too rare to
  run traffic on. Methalox, kerolox, hydrolox and uranium are in the physics sheet, and become goods
  with the engines and reactors that burn them.
