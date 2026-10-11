# Ground appearance inputs in the registry (inventory, 2026-10-10)

For planets' scenery pilot (task 20261010T174927-planets-1a84). What the registry holds today that could colour or
texture a ground face carrying `ground-vocabulary.rock@1`; nothing added or invented here. Paths under
`~/git/universe-fso/standards/`.

| Input | Where (key / field) | Per | Confidence | Usable with rock@1? |
|---|---|---|---|---|
| Rock unit identity | `Celestial/metadata/rock-units/*.yaml` (`rock-unit.*`), mapped by `Celestial/metadata/ground-vocabularies/rock.yaml` | rock unit (20) | sourced from the planet simulation | yes: the key |
| Density, magnetic susceptibility, erodibility | `rock-unit.*` `physical.*` | rock unit | sourced from the simulation (`basis`: tier sourced, review) | not appearance |
| Rock colour | **missing in the registry**; the simulation has an RGB per unit (planet-sim `stages/geology.py` UNITS, 4th field) that the import did not bring | rock unit | artistic (a map colour, not measured) | not yet |
| Albedo of a rock | **missing** for rock units | rock unit | | no |
| Surface roughness | **missing** everywhere | | | no |
| Asteroid rock classes: albedo, colour | `Celestial/metadata/rock-classes/*.yaml` (`rock-class.*`: carbonaceous, stony, metallic, icy, ...) `physical.albedo`, `physical.colour` | small-body class (9) | game figures (sourced from the game, chosen there) | no: small bodies, not ground units |
| Body albedo | `Celestial/schema/body.schema.yaml` `physical.albedo`; e.g. Hoar `treistun-e.yaml` 0.3 | whole body | curated/derived | a global tint only |
| Body colour | body `surface.colour` (RGB 0-1); Hoar [0.3, 0.6, 1.0] | whole body | artistic (the map's) | a global tint only |
| Regolith depth | body `crust.regolith` (m) | whole body | per record | soil/cover depth, global |
| Ice and ocean cover | body `water.ice_cover`, `water.ocean_cover` (share of surface) | whole body | per record; Hoar: a snowball, ice pole to pole (record text) | snow/ice share, global |
| Climate | body `surface.mean_temperature` (Hoar 216.2 K), `surface.temperature_low/high`, `surface.rain`, `atmosphere.*` (pressure, greenhouse, wind) | whole body | the record's (the owner keeps Hoar's canon) | climate, global |
| Soil / vegetation cover | **missing** (soil appears only in industry: grow halls, makers) | | | no |

## In short

- Per rock unit the registry has identity and physical properties, but **no colour, albedo or roughness**. The nearest
  thing is the simulation's own artistic RGB per unit, which could be imported labelled as artistic (registry), if you
  want it: say so and it is one import, no invented values.
- Snow, ice, regolith and climate exist only **per body**, not per place.
- Per-place cover (snow line, soil, vegetation) is the simulation's to produce, as its deposits are; the registry would
  hold only the vocabulary (as it does for rock), once there is one.

Schema needs, if any, go to engine before anything is added (as asked): none proposed here.
