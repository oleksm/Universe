# Spirit: what a settlement's top rung is made of

2026-10-06. Few recipes, many places (the civilization plan, part 7).

| Need | Met by | Records |
|---|---|---|
| Drink and company | taverns and inns | `building.tavern` (one to 500), `building.inn` |
| Play and sport | theatres, arenas, parks | `building.theatre`, `building.arena` (one to 20,000), `building.park` (two hectares to 5,000) |
| Art | artists where they work: studios and theatres. Art is a service here, not a stock: nothing is sold as "art" until an artwork is a described item | `building.studio` (ten artists to 10,000); `need.art` no longer takes a market category |
| News | the press | `building.press` |
| Schooling | schools, colleges, the flight school | `building.school`, `.college`, `.flight-school` |
| Travel for its own sake | **sights**: places worth going to see, from the runs' own figures | `Celestial/metadata/sights/`: the Spire of Heath (8,920 m), Hoar's Horn (5,045 m), the Ice of Hoar, the Great Basin of Cinder (3,758 km across, 2.3 km deep), the Rings of Drum |
| Work | a job: the census counts who has one; the game pays the wage | `settlement.census` |
| Worship | later, on the user's word | — |

A sight (`sight.schema.yaml`) says what it is (peak, basin, crater, canyon, ice, rings, storm,
sea, falls, aurora), on which body and where, its figures (height, diameter, depth) and the run's
id for it, and where it is best seen from. The figures are the simulation's: a peak from the bake's
`peaks.json`, a basin from the survey's `impacts.json`; the names are the registry's. As worlds are
run, their sights follow: Rime's, Anvil's, the moons'.
