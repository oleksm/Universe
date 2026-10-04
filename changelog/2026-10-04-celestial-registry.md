# The celestial registry

The seeded world written down, so it can be curated and not only seeded.

- **A fourth root in Freefall Facts**, `standards/Celestial`: the galaxy (seed 1984, home, the laws
  as the code has them), and the home system and the four stars its gates reach written out: 5
  systems, 59 bodies (planets, moons, named asteroids) and 26 asteroid fields.
- **Each record has a status**: seeded (as the seed makes it), curated (changed by hand: the truth,
  never written over) or frozen (curated and not to be changed, for planets whose landscape is made).
- **`cargo run -p universe-world --example celestial_export`** prints what the seed makes of those
  systems; **`python3 tools/standards/celestial_export.py`** writes it into the registry, leaving
  curated and frozen records alone and listing anything the seed no longer makes.
- On the registry page: the Celestial Registry with each system's star, planets, moons and fields;
  a body in Local Administration links to its celestial record; a Celestial report checks the two
  agree.
- The game does not read these records yet. That is the next step.

## The game reads it (the same day)

- **Curated and frozen bodies are taken from the registry**: the game makes each system from the seed
  as before, then a body whose record is curated or frozen is as the record says (mass, radius, day,
  orbit size and shape, rings, terrain, relief, air, colour). What goes round a body whose mass
  changed follows its new pull. Ports, fields and gates are placed after, on the world as curated.
- **The charted world can't change unnoticed**: a test makes the five charted systems from the seed
  and holds every seeded body to its record. A change to how worlds are made that would change one
  fails it.
- `content/base/celestial.ron` is written by the registry build; `crates/world/src/celestial.rs`.
- Not yet taken from records: the star itself, inclination and tilt, asteroid fields, bodies added or
  removed, and the laws.
