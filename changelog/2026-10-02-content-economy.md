# Economy content as data (C0.2–3)

The economy's content moved from Rust tables into the base pack:

| File | What's in it |
|---|---|
| `goods.ron` | kinds of goods: name, price range, mass, stowage density, what a thousand people use a day, and the word lists the catalogue's names are drawn from |
| `ores.ron` | what asteroids yield |
| `recipes.ron` | the works |
| `places.ron` | kinds of place: population, fuel kept for ships, works, and what unsettled markets of that kind sell and want |
| `markets.ron` | ban chances, rolled in order |

- **The code keeps only behaviour.** A facility's kind of place follows from where it is; fuel is
  the one kind of goods the code names (ships refuel from it).
- **`Category` is a handle to a kind of goods** now, not an enum. The economy's fixed 20-wide
  tables are sized to the content, so a new kind of goods is a data entry.
- **References are resolved at load:** recipes and places name kinds of goods and recipes by key,
  and a bad reference is refused with the reason. Every ore the asteroids are made of must be
  present.
- **Verified unchanged:** a dump of the whole goods catalogue, the markets of 16 systems, and the
  economy after 30 days is identical to the previous build's (5,789 lines).
