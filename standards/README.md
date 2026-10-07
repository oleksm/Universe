# Standards: the source

The standards, as YAML: edit here, in your editor; the game and a browsable page are built from
it (`docs/standards.md` for what a standard is).

    python3 tools/standards/build.py     # check everything; write index.html and the game's content
    open standards/index.html            # browse (refresh after a rebuild)

## Layout

```
standards/
  common.schema.yaml            shared definitions: key, physical, made_from, basis, revision, mark, batch
  dictionary.schema.yaml        the registry's shared words (enums with x-values)
  organisation.schema.yaml      companies, standards bodies, administrations, bands
  root.schema.yaml              a root's own record
  changes.yaml                  the tracker: every change to a record (tools/standards/tracker.py)
  game-keys.yaml                the registry's keys against the game's old names
  sources/                      research_*.json: cited figures with their sources and quotes
  SFO/                          the Standards Foundry Office: made things
    schema/                     one schema per record kind (equipment, hull, part, module, good, mill-stock, mount, building, market, structure, gate, standard)
    icons/                      <a record's file name>.svg: its icon (24 x 24 line art, stroke currentColor)
    metadata/
      SFO.yaml                  who it is
      0001-mining-ship.yaml     a standard: NNNN-<slug>.yaml, its id "SFO 1"; a standard may own a records folder (`records: elements`)
      elements/ materials/ goods/ stock/ mill-stock/ modules/ equipment/ mounts/ hulls/ parts/<hull or equipment>/ structures/ gates/ buildings/ markets/
  MakerHouse/metadata/makers/   the companies (organisation.schema.yaml)
  LocalAdministration/          administrations and their settlements: zones, parcels, streets, power lines, facilities (schema/, metadata/administrations/<system>/<settlement>/)
  Celestial/                    the seeded sky: systems and bodies, small bodies, rock classes and units, deposit types, vocabulary, sights, seeding; surveys/ (installed worlds, tools/standards/world_install.py)
  People/                       needs and professions
  Dogma/                        the physical laws and measures the engine's constants are generated from
  Engine/                       how the engine governs itself: metadata/scheduling/<clock>.yaml
```

- **Numbers are permanent:** `SFO 1` stays `SFO 1` whatever it's later filed under (as ISO
  numbers do). Take the next free one for a new standard.

## A standard

| Field | Holds |
|---|---|
| `version` | from 1 |
| `title`, `scope` | what it is; what it covers and what it doesn't |
| `purpose` | why it exists, a paragraph |
| `details` | `text` and/or a `table` (`columns`, `rows`) |
| `status` | `draft`, `published`, `superseded`, `withdrawn` |
| `sections` | titled sections, each `text` (paragraphs) and/or a `table` (`columns`, `rows`): for a standard that's mostly prose, a definition say |
| `refs` | ids it builds on (`"SFO 1"`) |
| `params` | `key`, `value` (a number, `[min, max]`, or text), `unit`, `note`; a key `ROW.column` (`S.length_max`) makes a table |
| `requires` | `subject` (`module.mass`), `check` (`at_most`, `at_least`, `equals`, `fits_within`, `provides`), `param`, `per` (the row: `size`, `class`...) |
| `text` | why, for people |
| `licence` | `open`, or `{fee: 500}` |

`identity` (key) and `title` are required; `parent` files it under another standard; `records` names the folder of records it owns; write what's known, leave the rest out. Ordinary case (the game shows capitals).

With a problem the build lists them all, the page shows them, and the game's content is left as
it was. Generated, not to edit by hand: `standards/index.html`, the eight `content/base/*.ron` files that start `// GENERATED` (bodies, brands, celestial, galaxy, industry, rock_classes, settlements, standards), and the engine's registry types and Dogma constants (crates/registry/build.rs, crates/physics/build.rs). Hand-kept beside them: aliases, prices, shapes, sheet.
