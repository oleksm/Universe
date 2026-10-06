# The survey contract: a simulated world into the registry and the game

How a world grown by the planet simulation (`~/git/planet-sim`) becomes a body people fly to,
land on, survey and mine. Three owners, no overlap:

| | Owns | Holds |
|---|---|---|
| **Registry** (this repository) | a body's identity and the inputs a run is seeded from; the shared words | `standards/Celestial/metadata/systems/<system>/bodies/<body>.yaml`; deposit types, rock units, rock classes, survey methods |
| **Survey** (the simulation's output) | everything the run derived | `standards/Celestial/surveys/<world_id>/` (a copy of the run's `worlds/<world_id>/survey/`) |
| **Game** | what people do with it | claims, mines, what each player has found, the render bake |

A run grows a world once and writes its survey once, read-only. A new run is a new world with a
new ID. Nothing moves a deposit after that: a claim pointing at `E4-PCU-010984A-01` points there
for good.

## 1. Seeding a run from a record (the simulation reads)

The body's record gives the run its inputs. Everything in these sections is the registry's:

| Section | Fields the run takes |
|---|---|
| `identity` | `key`, `name`, `kind`, `parent` (the star or planet) |
| `orbit` | `semi_major_axis`, `eccentricity`, `period`, `inclination` |
| `physical` | `mass`, `radius`, `gravity`, `day`, `tilt`, `albedo` |
| `bulk` / `interior` | composition, formation zone, where given |
| the star's record (`parent`) | luminosity, temperature, age |

The run writes `body: <key>` into `summary.json`, and the survey's `radius_m` must be the record's
`physical.radius` (within 1%); the import refuses a world not grown from the record. Our test:
world E4 is Earth-sized, Treistun d is 6,654 km; the import refuses to join them, as it should.

The vocabulary the run uses for deposit kinds, rock units and survey methods is the registry's,
through `docs/vocabulary.json` → `tools/standards/geology_import.py` → `Celestial/metadata/deposit-types/`,
`rock-units/`. The manifest hashes the vocabulary; a run on a vocabulary the registry has not imported
is a run to redo.

## 2. Importing a survey (the registry's tool)

    python3 tools/standards/survey_import.py <survey folder> <body key>
    python3 tools/standards/build.py

`tools/standards/survey_import.py`:

1. **Checks** the survey: format `planet-sim-survey/1`, `immutable: true`, every file present and
   its SHA-256 as the manifest says, `summary.body` the body (or none), the radius the record's.
2. **Copies** it to `standards/Celestial/surveys/<world_id>/`, read-only. A different survey under
   the same world ID is refused.
3. **Points the record at it**: `provenance: baked`, and a `survey` section with the world ID, the
   folder, the manifest's hash, the simulator's commit, when it was written, and the summary's
   headline figures (land share, counts of deposits, districts and belts, and the body's whole
   endowment by commodity, in kg).
4. A body already baked stays baked unless `--replace` is given; the old survey folder stays, since
   claims may point into it.

The record keeps its inputs. Where it held a seeded guess for a figure the run derives (mean
temperature, terrain, relief, ocean cover), the survey's figure is the truth; the next step of the
tool (owed, once a real run exists to read them from) replaces those.

Validation: `survey` is on the body schema (required fields, SI units, closed objects); `baked` is
in the dictionary; `build.py` accepts it and the tracker records the change. The survey folder
holds no YAML, so neither walker reads it as records.

## 3. What the game reads (the integrator)

**By reference.** The registry is the only copy the game loads; a baked body's record carries
`survey.folder`, and the game reads the files there. Nothing is copied into `content/`.

| File | What | For |
|---|---|---|
| `manifest.json` | hashes | check `survey.manifest_sha256` at load; refuse a mismatch |
| `summary.json` | the body's figures | the planet's information card: land share, endowment |
| `geology.png` | surface rock unit, equirectangular 2048 × 1024, one colour per rock unit | the ground's colour and material by place (rock unit → `Celestial/metadata/rock-units/<unit>.yaml`: density, strength, what it yields); a map layer |
| `bulk_rock.json` | each rock unit's land area and what it yields (aggregate, dimension stone, lime, clay) | what a quarry anywhere on that unit produces |
| `deposits.geojson` | one Point per deposit: `id`, `district_id`, `belt_id`, `kind` (→ `deposit-types/`, by `sim.key`), `tonnage_mt`, `grades` (per commodity, in the survey's units), `depth_m`, `blind`, `strike_deg`, `dip_deg`, `length_m`, `width_m`, `shape`, `host` (rock unit), `chance`, `seen_by` (survey method → whether that method finds it from above), `water_depth_m` (under the sea) | mining: a mine is placed on a deposit `id`; what it yields is the deposit's grades times what the `mine` module's recipe draws; prospecting: a ship's instrument of kind M reveals deposits whose `seen_by[M]` is true within its range; a map layer per method |
| `districts.json` | each district: ID, kind, its deposits, extent | the map's district layer; the land register's unit for a mining licence |

Coordinates: longitude, latitude in degrees on the body's sphere; the rock map's x is longitude
−180 → 180 left to right, y is latitude 90 → −90 top to bottom.

**Claims and mines** (`LocalAdministration`): a claim's `deposit` is a deposit `id`; its `world_id`
is the first segment of the ID. A claim's deposit must exist in the body's survey, and the body
must be baked: the validator checks both once claims are records.

**The render bake** (landscape, 600 m tiles, rivers) is the game's and the lab's, outside this
contract: it reads the world and changes nothing in the survey. The registry only needs to know
where the bake is, if the game needs a pointer; `surface.landscape` already exists for that.

**Map layers, not views:** rock units, districts and belts, and deposits by method are toggles
over the planet map that exists.

## 4. Later layers, same pattern

Each is one more derived file in the survey, one more vocabulary in the registry, one more layer
in the game: **air** (gases and pressure, replacing the hand-set `breathable`), **weather**
(wind, rain and seasons by region, replacing the game's one climate formula), **foliage** (a biome
map and a `biome` vocabulary; farm yields tied to ground), **animals** (a species vocabulary and
where each lives).

## Open

- The run of Treistun d (then e, f): the first real test of 1 and 2.
- The derived figures the record drops once a survey exists (temperature, terrain, relief, ocean
  cover): the tool's next step, written against the first real run.
- Size: a survey is about 3 MB (E4; the GeoJSON is 2.6 MB of it). Three or four worlds in git is
  fine; a hundred is not. If the galaxy's bodies are all run, the survey folders move to a store
  the game fetches by hash, and `survey.folder` becomes a URL. Not yet.
