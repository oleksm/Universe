# 2026-10-06: the survey contract

- **A body can be baked:** `provenance: baked` (dictionary), a `survey` section on the body schema
  (world ID, folder, manifest hash, simulator commit, headline figures, the endowment in kg).
- **`tools/standards/world_install.py`:** checks a simulation's survey against its manifest, copies it
  read-only to `standards/Celestial/surveys/<world_id>/`, points the body at it. Refuses a world not
  grown from the record (radius), a different survey under a known ID, or re-baking without
  `--replace`.
- **`docs/survey-contract.md`:** what the simulation reads from a record, what the import does, what
  the game reads from the survey and for what, the later layers (air, weather, foliage, animals).

# 2026-10-06: planets named, and matched by ID

- **Names:** the planets of Treistun are named by their people: Cinder, Anvil, Harvest (d, the
  farm world), Hearth (e, the home world), Rime, Drum, Pale, Hush, Verge; moons follow their
  planet. `identity.also` keeps the seed's letter. 28 records reworded.
- **The body's key is its ID.** The seed's bodies carry `key` (`system::Body::key`, slugged from
  the seed's name once, `StarSystem::key_bodies`); the registry's records match by key, not name,
  and a matched body takes the record's name (`celestial::apply`). Settlements find their body by
  key. A rematch test over the charted region: 109 records matched by key, 24 of them renamed.
- **Seedable inputs** a planet run wants, on the body schema: `interior.radiogenic`,
  `water.mass_share` (and `physical.age` on the star, which existed).
- **Harvest moved out:** 0.408 → 0.525 AU (year 165 days), with the planet lab: at 0.408 AU it took in
  422 W/m2 against a runaway limit of about 282 and could keep no sea. Its mean temperature is the
  run's to give and is off the record. Star's age, water share and radioactive heat written as the
  lab's defaults, chosen. World IDs: TRD1 scheme. The contract looks the rock map up in
  `rock_units.png`.
- **Harvest baked:** world TRD1 imported; the record points at `standards/Celestial/surveys/TRD1/`.
- **Ring crossing:** a step ending within 1 mm of a gate's plane crosses it (the ring's acceleration
  over a step put the carried start on the wrong side; the gate test hit it exactly).

# 2026-10-06, later: the world's three packages

- `world_install.py` (was `survey_import.py`) takes the world folder: survey and energy copied,
  the surface bake pointed at in a worlds store (`bake` on the record). Harvest has its energy
  package (46,672 oil and 20,477 gas fields in 282 basins, 89 coalfields) and its surface v1.
- Deposit types oil field, gas field, coalfield (formed by burial); goods crude oil, natural gas,
  coal with bulk stock; modules oil well, gas well, coal mine.
- The body's life timeline (`life.began` and its steps) and `surface.highest` from the run.
- **The worlds store and its index** (`planet-sim-releases/1`): `world_install.py --store <root>`
  installs every current world with a body; superseded worlds stay history. Files over 20 MB (an
  airless world's `impacts.json`) stay in the store, listed as `in_store` on the record.
- **Cinder (Treistun b) baked from world TRB1**: 180 deposits, all blind, 17 districts; its
  surface v1 (1.0 GB) in the store.
- The lab's vocabulary of 2026-10-06 imported: 14 new deposit types (skarns, lithium pegmatite,
  IOCG, Carlin and placer golds, two uraniums, rare-earth clay, graphite, three impact kinds) with
  ore goods, bulk stock and mine recipes; 7 new rock units (an airless world's: primary crust,
  intercrater plains, high-titanium basalt, impact melt and breccia, anorthosite, KREEP basalt);
  events `impact` and `river`.
- **A Worlds report** on the page: every baked body with its packages (energy "none" where a world
  never had life), its surface bake and what stays in the store; below, what the store lists that is
  not installed and why. The installer ends a `--store` run with the same table.
- **`watch_worlds.py`**: watches the store's index; installs, validates, builds, logs the release to
  `docs/worlds-releases.md`, stages for review; commits only with `--commit`, never pushes.
- **Surface v2 installed for Harvest and Cinder** by the watcher: `bounds` in fz.json and the
  climate block; the records take the run's mean temperature, coldest and warmest months, rain and
  refined peak (Harvest 298 K, 0.68 m of rain, 8,920 m; Cinder 474 K, 4,713 m). Airless worlds'
  heights are relative to the mean radius; noted in the contract.
