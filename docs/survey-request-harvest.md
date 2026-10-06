# Request to the planet lab: run Harvest (Treistun d)

From the world scientist, 2026-10-06. Please grow Harvest, the farm world of Treistun, from its
record, and write its survey. The contract is `docs/survey-contract.md`; the record is
`standards/Celestial/metadata/systems/treistun/bodies/treistun-d.yaml` (key
`body.treistun.treistun-d`; its people call it Harvest, the surveyors Treistun d).

## The star: Treistun (`body.treistun.treistun`)

| Input | Value | In your units | Source |
|---|---|---|---|
| mass | 1.4025e30 kg | **0.705 suns** | the seed |
| luminosity | 1.127e26 W | **0.294 suns** (K class) | the seed; use it, not mass^4, which would give 0.25 |
| age | not on record | **your default**; the seed does not say. If you need one: a K dwarf at 0.7 suns that has settled a system with a 1.07 g world, anything from 2 to 8 Gyr is open. Say what you took and I will write it on the star's record as `physical.age`, tier chosen |
| metallicity [Fe/H] | not on record | **0** (your default) |

## The planet: Harvest (`body.treistun.treistun-d`)

| Input | Value | In your units | Source |
|---|---|---|---|
| mass | 6.9752e24 kg | **1.168 earths** | the seed |
| radius | 6,654,110 m | **1.0444 earths** | the seed (density 5,650 kg/m3, gravity 10.51 m/s2) |
| semi-major axis | 6.10084e10 m | **0.4078 AU** | the seed |
| eccentricity | **0.04874** | | the seed |
| obliquity | **13.73 deg** | | the seed |
| rotation | 72,612 s | **20.17 h** | the seed |
| radiogenic heat | not on record | **1.0** (your default) | the record has no bulk composition yet |
| surface temperature, as a start | 321.6 K | the seed's figure for the mean; **your climate stage owns the true value**, and the record will take yours |
| regime | not on record | **your default** (plates); the lore has mountains and seas, which is plates' doing |
| core fractions, stratified | not on record | **your defaults** |
| water, share of mass | not on record | **your choice, in Earth's range (2.3e-4) or a little under**: the lore wants seas and continents (`surface.terrain: terran`) with fields between 25 and 50 degrees of latitude, so land there. Say what you took and it goes on the record as `water.mass_share`, tier chosen |
| formation zone | 0.41 AU round a 0.29-sun star: **warm**, as E4 | — |
| seed | **yours**; write it in the manifest |

What the lore asks of the result, so you can see whether the run agrees (and if physics says no,
the lore changes, not the run): a mean of about 320 K; an equator too hot for crops and frozen
poles; land to farm in two bands, 25 to 35 degrees (hot crops) and 38 to 50 (temperate); air as
dense as Earth's, breathable; one moon, Harvest I (1,527 km radius, which your run can ignore).

## What to write

`worlds/<world_id>/survey/` as the contract says, with `summary.body: body.treistun.treistun-d`.
The import checks `radius_m` against the record's 6,654,110 m within 1%: a world of another radius
is refused. Our vocabulary import is current with your `docs/vocabulary.json` (its hash is in your
manifest; if you have changed it since, tell me and I re-import before you run).

Then I run `python3 tools/standards/survey_import.py <folder> body.treistun.treistun-d`, Harvest
becomes `baked`, and the game reads your rock map, deposits and districts from the registry.

## After Harvest

Hearth (`treistun-e`, the home world: 6,281 km, 0.98 g, 264 K, 0.66 AU, ice and air) and Rime
(`treistun-f`: 3,634 km, 0.41 g, 189 K, 1.13 AU, airless) next, in that order.
