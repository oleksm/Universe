# Dogma read from the registry

- **The engine's laws come from the Dogma registry** (`standards/Dogma`), not `config/dogma.ron`, which
  is gone. The physics crate's build reads the 31 laws' records and makes `universe_physics::laws`
  from them: one constant per law, under its label, in SI as the record holds it. Change a law's
  record, rebuild, and the engine has it. There's no other copy.
- **All in SI.** The best field speed is in m/s (it was a multiple of c) and a tube's crossing time in
  s per metre (it was per light year). The hyper laws work in those units directly.
- **The measures are Dogma's.** `universe_world::units` (G, AU, light year, day, year, the Sun's and
  Earth's mass and radius) are names for Dogma's laws, not numbers of their own. The game's own speed
  of light, the economy's own day, and the world sheet's Sun luminosity, air heat capacity, lapse rate
  and four albedos are gone in favour of the laws.
- **Numbers that had no name now use one.** Earth's air density, its scale height and where its air
  is taken to end, and one standard gravity: **9.80665 m/s²**, where the code wrote 9.81 in nine places
  (magnetic boots, walking tests, the gravity read-out on foot, the hover test). Everything that used
  9.81 is 0.03% lighter.
- `docs/physics-sheet.md` is regenerated from the registry.
