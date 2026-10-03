# Standards: the source

The standards registry, as YAML: edit here, in your editor; the game and the browsable page are
built from it (`docs/standards.md` for what a standard is).

    python3 tools/standards/build.py     # check everything; write index.html and the game's content
    open standards/index.html            # the tree to browse (refresh after a rebuild)

## The tree is the folders

```
standards/
  FSO/                                  a body: its folder named after its prefix
    _body.yaml                          who it is, where it sits
    2-mechanical-interfaces/            a branch: <path>-<slug>
      _branch.yaml                      its path (2) and title
      2.1-sockets/                      a sub-branch, inside its parent
        _branch.yaml
        001-hardpoint-socket.yaml       a standard: NNN-<slug>; its id is FSO/2.1/001
  schema/                               JSON Schemas for your editor
```

- A new branch: a folder `<path>-<slug>` with a `_branch.yaml` (`path`, `title`, optional `note`),
  inside its parent's folder.
- A new standard: `NNN-<slug>.yaml` in its branch (copy one). Its id comes from where it is.
- Each file's first line points at its schema, so an editor with the YAML language server (VS
  Code's Red Hat YAML extension, for one) completes fields and flags mistakes as you type.

## A body (`_body.yaml`)

`key` (body.fso), `name`, `prefix` (the folder's name), `seat` (a place, or `home`), `note`
(one line), `kind` (`consortium`, `independent`, `authority`, `corporation`, `players`),
`founded_by` (brand keys from `content/base/brands.ron`), `about` (paragraphs: who it is, why it's
followed, what it isn't).

## A standard

| Field | Holds |
|---|---|
| `version` | from 1 |
| `title`, `scope` | what it is; what it covers and what it doesn't |
| `status` | `draft`, `published`, `superseded`, `withdrawn` |
| `refs` | ids it builds on (`FSO/0.1/001`) |
| `params` | `key`, `value` (a number, `[min, max]`, or text), `unit`, `note`; a key `ROW.column` (`S.length_max`) makes a table |
| `requires` | `subject` (`module.mass`), `check` (`at_most`, `at_least`, `equals`, `fits_within`, `provides`), `param`, `per` (the row: `size`, `class`...) |
| `text` | why, for people |
| `licence` | `open`, or `{fee: 500}` |

Write in ordinary case: the game shows it in capitals. Notes on invented numbers say what
they're aimed at.

The build checks what the schema can't too (references found, branches nested in their parents'
folders, requirements against parameters that exist, numbers unique, ranges the right way up).
With a problem it lists them all, the page shows them, and the game's content is left as it was.

Generated, not to edit by hand: `standards/index.html`, `content/base/bodies.ron`,
`content/base/standards.ron`.
