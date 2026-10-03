# Standards: the source

The standards, as YAML: edit here, in your editor; the game and a browsable page are built from
it (`docs/standards.md` for what a standard is).

    python3 tools/standards/build.py     # check everything; write index.html and the game's content
    open standards/index.html            # browse (refresh after a rebuild)

## Layout

```
standards/
  FSO/                          a body: its folder named after its prefix
    FSO.yaml                  who it is (key, name, prefix, seat, kind, founded_by, about, note)
    schema/                     the schemas for the editor
    metadata/
      0001-mining-ship.yaml     a record: NNNN-<slug>.yaml, its id "FSO 1"
```

- **Numbers are permanent:** `FSO 1` stays `FSO 1` whatever it's later filed under (as ISO
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
| `refs` | ids it builds on (`"FSO 1"`) |
| `params` | `key`, `value` (a number, `[min, max]`, or text), `unit`, `note`; a key `ROW.column` (`S.length_max`) makes a table |
| `requires` | `subject` (`module.mass`), `check` (`at_most`, `at_least`, `equals`, `fits_within`, `provides`), `param`, `per` (the row: `size`, `class`...) |
| `text` | why, for people |
| `licence` | `open`, or `{fee: 500}` |

Only `version`, `title`, `status`, `scope`, `text`, `licence` are needed; write what's
known, leave the rest out. Ordinary case (the game shows capitals).

With a problem the build lists them all, the page shows them, and the game's content is left as
it was. Generated, not to edit by hand: `standards/index.html`, `content/base/bodies.ron`,
`content/base/standards.ron`.
