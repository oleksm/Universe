# PlanetSim ground catalogue contract

Implements planet-forge `docs/catalogue/README.md` sections 1–3 (94e4c4f),
with the emitted-kind manifest added in 903ae35. Planets owns the fill pass;
Registry owns schemas, the ladder, cross-checks and their integration.

## Eight view decades

| Key suffix | View across (m) | Target ground pixel (m) |
|---|---:|---:|
| planet | 10000000 | 10000 |
| continent | 1000000 | 1000 |
| region | 100000 | 100 |
| landscape | 10000 | 10 |
| site | 1000 | 1 |
| slope | 100 | 0.1 |
| ground | 10 | 0.01 |
| surface | 1 | 0.001 |

These are view/pixel design targets, not a claim of achieved sampler resolution.
Surface is texture only. Old context links map planet→planet, cluster→continent,
pattern→region, square→landscape, detail→site. No old sample counts or segment IDs
are copied to the new levels. Existing feature `levels` remain context hints;
new draw ranges use `ladder`, never a guess from the old numeric `level`.
The obsolete numeric feature field is removed without manufacturing size data.

For `size.across_m: [minimum, maximum]`, coarsest is the first descending view
where maximum >= view/100. Finest is the last view >= minimum. Boundaries are
inclusive. Oversized largest members can extend beyond a view if smaller members
fit; a minimum larger than every view or maximum smaller than 1 cm has no valid
interval and is a gap. Below the finest view, only children supply detail.
The context helper uses every level in the inclusive derived interval once set.

## Authoring records

Use `python3 tools/standards/reg.py new planet-feature <slug>` and its `set`
command. The closed schema is the exact field contract:
`standards/PlanetSim/schema/planet-feature.schema.yaml`.

- `size` holds positive ordered metre ranges `across_m`, optional `length_m` and
  `relief_m`, plus a nonempty `basis` array. Measurements are sourced; provisional
  ranges must be explicitly marked for review. No sizes are inferred in migration.
- `ladder` has exactly two `planet-level.*` keys derived by the rule above.
- `description` and `identity.about` describe the actual ground kind; old family
  placeholders are migration gaps.
- `shape` holds `plan`, `cross_section` and `long_profile` laws (`rule` plus named
  `parameters` from dimensions), and `blend`: max, min, step, level or material.
  Material applies only to cover. Level applies to water or an explicitly declared
  `flat_floor: true`; naming a flat floor does not supply a measured elevation.
- `children` and `parent_kinds` name ground feature kinds. `grows_into` is an array
  of `{kind, rule}` selections. Their combined parent→child/growth graph is acyclic;
  reciprocal parent/child declarations are one edge. Children cannot start coarser
  than their parents. All references must exist.
- `examples` hold name, lat/lon degrees, measured scalar metre `size` values,
  measurement `source`, and a `source_dem` object with name/source/optional
  resolution_m. Cover examples may use `source_data` (e.g. WorldCover) instead.
  Latitude/longitude ranges and this conditional provenance are validated.
- `setting` describes tectonic, rock and climate conditions. Reusable meanings
  remain separate from the simulator's actual feature instances.
- Feature `line_kinds` and line `feature_kinds` link meaning to geometry both ways.
  River↔trunk-river and divide↔drainage-divide are mandatory pairs and are supplied.

## Six checks and migration

`python3 tools/standards/planet_catalogue.py` checks:

1. Exact decade levels, size-derived ladder, descriptions and shape completeness.
2. References, child scale order and cycles in expansion/growth.
3. At least one example for built/landmark kinds; nonempty dimensions/spans for built kinds.
4. Every emitted simulator kind is registered.
5. Material/level blend restrictions.
6. Reciprocal geometry/meaning links, including the two mandatory pairs.

The registry build publishes `rep:planet-catalogue` and `reg.py check` runs the
same audit. During the agreed fill pass, missing catalogue data remains visible
as gaps; regular builds do not present them as complete or fail solely on gaps.
Provided values still must fit their schemas, SI units, finite/range checks and
reference kinds. `--strict` exits nonzero on any catalogue gap. After Planets
finishes the records, Registry will enable strict mode by default; that is a
separate delivery, not a claim made by this scaffold.

The registry ships `standards/sources/planet-feature-emitted-kinds.yaml`, a
versioned YAML snapshot of the forge JSON manifest. Registry checks do not depend
on a sibling checkout. The forge owns source freshness via
`python3 tools/emitted_kinds.py --check`. Use `--emitted-kinds <manifest>` to
check a newer explicit JSON/YAML manifest; update the versioned snapshot when
integrating new emitted kinds. `--features-jsonl <run-file>` also checks observed
run kinds, preventing a run-only unregistered kind from escaping the audit.
`--json` exports individual gaps by key and check number.
