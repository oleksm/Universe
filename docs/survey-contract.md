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
world E4 is Earth-sized, Heath is 6,654 km; the import refuses to join them, as it should.

The vocabulary the run uses for deposit kinds, rock units and survey methods is the registry's,
through `docs/vocabulary.json` → `tools/standards/geology_import.py` → `Celestial/metadata/deposit-types/`,
`rock-units/`. The manifest hashes the vocabulary; a run on a vocabulary the registry has not imported
is a run to redo.

## 2. Installing a world (the registry's tool)

    python3 tools/standards/world_install.py <world folder>        # worlds/<world_id>/, with survey/, energy/, surface/
    python3 tools/standards/build.py

`tools/standards/world_install.py`:

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
tool (`world_install.climate()`, reading the survey's summary) replaces those.

Validation: `survey` is on the body schema (required fields, SI units, closed objects); `baked` is
in the dictionary; `build.py` accepts it and the tracker records the change. The survey folder
holds no YAML, so neither walker reads it as records.

## 2a. The world's three packages

| Package | Written | Kept in the registry | On the body's record |
|---|---|---|---|
| `survey/` (planet-sim-survey/1) | once per run | copied whole (3 MB), files read-only | `survey` |
| `energy/` (planet-sim-energy/1): petroleum basins, oil and gas fields, coalfields; IDs permanent | once per run | copied whole (22 MB) | `energy`: counts, in-place oil (m³), gas (m³), coal (kg); model estimates calibrated on the simulation's Grown Earth, not recoverable |
| `surface/` (planet-sim-surface/1): the flyable bake, versioned; `latest.json` names the current | per bake, each version once | **not copied** (0.7 GB): `latest.json` and the version's `manifest.json` only | `bake`: version, path in the worlds store, manifest hash, files, bytes |

The install takes the world folder, finds the three, checks every file against its manifest, and
is idempotent. The surface's files live in a **worlds store**: a folder the game is configured with
(`worlds/<world_id>/surface/v<N>/`), today the lab's `planet-sim/out/`; the game fetches by the
manifest's hashes, so any copy of the store will do.

Deposit types for the energy kinds: `deposit-type.oil-field`, `.gas-field`, `.coalfield`
(formed by burial); goods crude oil, natural gas, coal and their bulk stock; modules `oil-well`,
`gas-well`, `coal-mine` drawing from the place.

A fourth package, **`history/`** (planet-sim-history/1), is how the world grew, for the game's
studio: `frames.json` and the globe frames over time (`t<NNNN>.jpg`, `today.jpg`), `params.json`
(the run's given, derived and assumed parameters), `history.json` (interior and plates every
10 Myr), `paleo.json` (climate every ~100 Myr: CO₂, temperature, ice, burial), `life.json`
(life's milestones). Written once per world; `releases.json` lists it under `packages.history`.
The registry keeps nothing of it (the installer needs nothing from it); the game's studio reads it
from the store; the Worlds report says whether a world has one.

## 2c. Installing a grown planet's ground: one command

A planet grown as lines (the planet graph, planet-unfold's renderer) goes to the game with one command, from the
planet's directory, run in the fso worktree:

    python3 tools/standards/install_world.py <planet-dir> --world=TRE3 --as=Hoar                  # dry run: what it would do
    python3 tools/standards/install_world.py <planet-dir> --world=TRE3 --as=Hoar --apply          # do it, validate, build
    python3 tools/standards/install_world.py <planet-dir> --world=TRE3 --as=Hoar --push           # and commit and push on fso

Options: `--survey=<id>` (default the `--world`'s, if the store has its survey), `--snapshot-name=<name>` (default
`<planet>-<root sha256, 8 hex>`), `--root=<file>` (default `ck_22_rivers_anchors.lines`; depth files
`L<level>_<i>_<j>.lines` beside it are taken too), `--engine=<lines.rs>` (default the reader the game's Cargo.toml
names), `--climate` (take the survey's climate; by default the record's is untouched), `--note=<text>`.

It checks every line file's PTL version and line kinds against the engine's reader (its `VERSION` and `Kind` enum) and
**refuses cleanly** while the engine does not read a kind the file carries, so the install waits instead of breaking the
game; makes the snapshot `worlds/<world>/ground/<name>/` in the worlds store (the root as `L0_0_0.lines`, the depth files,
and `manifest.json`: format `planet-unfold-tiles/1`, world_id, body, name, radius_m read from the file, l0, start,
source {commit, file, snapshot, date}, files with sha256 and size); points the body's record at it (survey and energy
from the survey world through `world_install.py`, `ground` through `set_ground.py`; radius, gravity, climate and lore
stay the record's); runs the validators and the registry build and puts the record back if either fails; with
`--push` commits the record and the survey folder on fso and pushes. A second run with the same files changes nothing.

## 2b. Installing without anyone: the watcher

`tools/standards/watch_worlds.py` watches the store's `releases.json`. When it has changed since
the last install it runs `world_install.py --store`, both validators and `build.py`, appends the
release to `docs/worlds-releases.md` (which world, for which body, its packages' hashes), stages
everything, prints the Worlds table and `git diff --cached --stat`, and sends a desktop notice.
**Nothing is committed unless `--commit`, and nothing is ever pushed**: the review is a person's.

    python3 tools/standards/watch_worlds.py --once            # check now
    python3 tools/standards/watch_worlds.py                   # every five minutes
    python3 tools/standards/watch_worlds.py --once --commit   # commit on the current branch, unpushed

To run it in the background on this machine (a user timer; the store is the lab's folder):

    # ~/.config/systemd/user/freefall-worlds.service
    [Service]
    Type=oneshot
    WorkingDirectory=%h/git/universe-fso
    ExecStart=/usr/bin/python3 tools/standards/watch_worlds.py --once
    # ~/.config/systemd/user/freefall-worlds.timer
    [Timer]
    OnBootSec=2min
    OnUnitActiveSec=10min
    [Install]
    WantedBy=timers.target
    # systemctl --user enable --now freefall-worlds.timer

The last installed index is remembered in `standards/Celestial/surveys/.installed.json`
(committed, so the review shows what the installer thought it had).

## 3. What the game reads (the integrator)

**By reference.** The registry is the only copy the game loads; a baked body's record carries
`survey.folder`, and the game reads the files there. Nothing is copied into `content/`.

| File | What | For |
|---|---|---|
| `manifest.json` | hashes | check `survey.manifest_sha256` at load; refuse a mismatch |
| `summary.json` | the body's figures | the planet's information card: land share, endowment |
| `rock_units.png` | the rock map programs look up: equirectangular 2048 × 1024, each pixel the rock unit's number (`summary.rock_units` says which), 255 for sea | the ground's material by place (rock unit → `Celestial/metadata/rock-units/<unit>.yaml`: density, strength, what it yields) |
| `geology.png` | the same map coloured for people, sea blended | the map layer; never looked up |
| `bulk_rock.json` | each rock unit's land area and what it yields (aggregate, dimension stone, lime, clay) | what a quarry anywhere on that unit produces |
| `deposits.geojson` | one Point per deposit: `id`, `district_id`, `belt_id`, `kind` (→ `deposit-types/`, by `sim.key`), `tonnage_mt`, `grades` (per commodity, in the survey's units), `depth_m`, `blind`, `strike_deg`, `dip_deg`, `length_m`, `width_m`, `shape`, `host` (rock unit), `chance`, `seen_by` (survey method → whether that method finds it from above), `water_depth_m` (under the sea) | mining: a mine is placed on a deposit `id`; what it yields is the deposit's grades times what the `mine` module's recipe draws; prospecting: a ship's instrument of kind M reveals deposits whose `seen_by[M]` is true within its range; a map layer per method |
| `districts.json` | each district: ID, kind, its deposits, extent | the map's district layer; the land register's unit for a mining licence |
| `energy/fields.geojson`, `basins.json`, `coalfields.json` | fields (oil or gas, in-place amounts, depth, trap, water depth), basins, coalfields (rank, area) | a well or a coal mine sits on a field's or coalfield's `id`; a map layer |
| the surface bake (`bake.path` in the worlds store) | 5 km and 600 m height tiles, rivers, textures, geology and energy layers, peaks, the world's report | the ground to fly over and land on; the globe; the map layers |
| the bake's `fz.json` (the 600 m tiles' index), field `bounds` | `{"<face>/<x>/<y>": [min_m, max_m]}` for every tile in `tiles`: the absolute heights (m above the sea) of the ground as the client rebuilds it (the 5 km map read bilinearly plus the tile's 16-bit difference, half-metre steps), min floored and max ceiled to whole metres; the highest bound equals the world's highest peak (Heath 8,920 m). In the lab's bakes from 2026-10-06 on; absent in surface v1 of TRD1 and TRB1, and absent means unknown | culling: skip a tile whose bounds lie out of view or below the sea |
| `climate` in the survey's `summary.json` (new surveys), or `surface/v<N>/summary.json` (older worlds, from their next bake) | `mean_surface_temp_c`, `mean_land_temp_c`, `rain_land_m`, and `bands` (0–10, 10–25, 25–35, 35–50, 50–70, 70–90°: `temp_c`, `land_temp_c`, `land_coldest_month_c`, `land_warmest_month_c`, `land_rain_m`, null on dry worlds); `highest_point_plate_stage_m` (the plate stage's ~80 km figure, not the summit). The bake's summary also carries `highest_peak` `{h, prom, lat, lon, res_m}`, the refined summit. One shared function (`planet_sim.climate_summary`), the same figures as the world report's weather table | the installer writes `surface.mean_temperature`, `temperature_low` and `temperature_high` (the coldest and warmest land months across the bands, in K), `rain` (m a year) and `highest` (the refined peak) onto the record; the game's climate and the planet's card read them |
| airless worlds (no sea) | heights and `bounds` are relative to the **mean radius**, not a sea (Cinder: −5,435 to +4,880 m); `highest_peak` comes from the 5 km peak list, while the 600 m tiles carry crater relief above it, so the bounds' max may exceed `highest_peak` (Cinder 4,880 against 4,713 m); on worlds with plates they agree (Heath 8,920 = 8,920) | the datum for landing and the map's heights; the installer takes `highest_peak` for `surface.highest` |
| the bake's air (surface v3 on): `atmosphere.json` beside `air_luts.json`, `air_transmittance.rgba32f`, `air_multiscatter.rgba32f` | the world's air for rendering: its scattering constants, and the precomputed tables (transmittance and multiple scattering, 32-bit float RGBA) the sky and aerial perspective are drawn from; the aerosol Earth's mean until a world's own is simulated | the game's air (worlds::Heights::air_luts); absent before v3 |
| the bake's clouds (surface v4 on, format planet-sim-clouds/1): `clouds.json` beside `clouds_month.png`, `clouds_enso.png`, `clouds_air.png` (and `globe_clouds.*` for people) | twelve monthly tiles (4 across × 3 down, 360 × 180 each, equirectangular, the world's own months) of sky fraction by kind in RGBA (low, deep convection, frontal, cirrus; v/255); the change per unit of its El Niño index; the air (cloud base 16·v m, tropopause 80·v m, 700 hPa and jet eastward winds); `kinds` gives each cloud's optical depth, base and top rules | the game's cloud layer and its weather by month (`clouds.json` says how to read every channel); absent before v4 |
| coming: sparse ~150 m tiles (cube level 6, the same naming and encoding, each a difference from the 600 m ground) | | a third height level, when the game reads one |

Coordinates: longitude, latitude in degrees on the body's sphere; the rock map's x is longitude
−180 → 180 left to right, y is latitude 90 → −90 top to bottom.

**Claims and mines** (`LocalAdministration`): a mine or well facility carries `claim: {deposit, holder, licence}`; the
deposit is the survey's or the energy package's permanent id (`TRD1-PCU-007264A-01`), the holder an organisation, the
licence the administration's (its `law.policies.mining_licence`; without one the mine is unlicensed, a recorded breach).
The build checks the id against the survey of the body the settlement is at, and that the body is baked. The first:
**Halden Camp** on Heath (`settlement.treistun.halden-camp`), Cormorant's pit and concentrator on a 1,230 Mt porphyry
copper body 1,070 km north of Port Eikir, its concentrate on the exchange at its strip's yard.

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

## World IDs

The deposits' random draws are seeded from the world ID, so the ID is part of the world and can
never change after the run. Scheme: three letters of the system, the body's seed letter, the run
number: **TRD1** is Treistun d's first run (deposit IDs `TRD1-PCU-…`), TRE1 Hoar's, TRF1 Rime's.
A moon takes its planet's letter and its numeral: TRDI1 for Treistun d I. A second run
of the same body is a new world, TRD2.

## Open

- The run of Heath (then e, f): the first real test of 1 and 2.
- The derived figures the record drops once a survey exists (temperature, terrain, relief, ocean
  cover): the tool's next step, written against the first real run.
- Size: a survey is about 3 MB (E4; the GeoJSON is 2.6 MB of it). Three or four worlds in git is
  fine; a hundred is not. If the galaxy's bodies are all run, the survey folders move to a store
  the game fetches by hash, and `survey.folder` becomes a URL. Not yet.
