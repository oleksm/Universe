# Standards authored as YAML; a page to browse them

- **The source moves to `standards/`** (YAML, in the repo): the folders are the tree (a body,
  its branches as nested folders, a standard per file; ids from where they sit), with JSON
  Schemas for the editor. The FSO's ten moved over.
- **`python3 tools/standards/build.py`** checks everything (fields, references, branch nesting,
  requirements against parameters, numbers, ranges) and writes `standards/index.html` — the
  tree to browse, search, parameter tables with headers, what each builds on and what builds on
  it — and the game's `bodies.ron` and `standards.ron` (generated now). With a problem: all
  listed, on the page too, the game's content left as it was.
