# Standards: the source

The standards, as YAML: edit here, in your editor; the game and a browsable page are built from
it (`docs/standards.md` for what a standard is).

    python3 tools/standards/build.py     # check everything; write index.html and the game's content
    open standards/index.html            # browse by topic or by number (refresh after a rebuild)

## Layout

```
standards/
  FSO/                          a body: its folder named after its prefix
    _body.yaml                  who it is (key, name, prefix, seat, kind, founded_by, about, note)
    0001-size-classes.yaml      a standard: NNNN-<slug>.yaml, its id "FSO 1"
```

- **Numbers are permanent:** `FSO 1` stays `FSO 1` whatever it's later filed under (as ISO
  numbers do). Take the next free one for a new standard.
- **Topics are free tags** (`topics: [vessels, docking]`), as many as describe it. The page
  groups by them; how they'll be classified (likely more than one dimension) is left until
  patterns show. Lower case, words joined by `-`.

## A standard

| Field | Holds |
|---|---|
| `version` | from 1 |
| `title`, `scope` | what it is; what it covers and what it doesn't |
| `status` | `draft`, `published`, `superseded`, `withdrawn` |
| `topics` | at least one tag |
| `refs` | ids it builds on (`"FSO 1"`) |
| `params` | `key`, `value` (a number, `[min, max]`, or text), `unit`, `note`; a key `ROW.column` (`S.length_max`) makes a table |
| `requires` | `subject` (`module.mass`), `check` (`at_most`, `at_least`, `equals`, `fits_within`, `provides`), `param`, `per` (the row: `size`, `class`...) |
| `text` | why, for people |
| `licence` | `open`, or `{fee: 500}` |

Only `version`, `title`, `status`, `topics`, `scope`, `text`, `licence` are needed; write what's
known, leave the rest out. Ordinary case (the game shows capitals).

With a problem the build lists them all, the page shows them, and the game's content is left as
it was. Generated, not to edit by hand: `standards/index.html`, `content/base/bodies.ron`,
`content/base/standards.ron`.
