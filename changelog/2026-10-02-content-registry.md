# Content registry (C0.1)

The start of content as data (design: `docs/content.md`).

- **Packs:** RON folders. The base pack (`content/base/`) is built into the binary; override
  packs named in `UNIVERSE_CONTENT` (`:`-separated, in order) add entries or replace them by key.
- **Registry** (`world::content`): loaded once, validated, shared read-only (`content()`).
  Entries are reached by typed `Handle`s; they're stored and sent as their **key**
  (`hull.cobra`), with renamed keys resolving through the packs' `aliases.ron`. One stable
  **hash** (FNV-1a over the packs' sources) for saves and peers to compare.
- **Validation at load:** unsound entries are refused with the reason, e.g. a hull without a
  main drive, or a negative mass.
- **Hulls are the first content:** the Cobra's numbers moved from code to
  `content/base/hulls.ron`. A ship's hull is a handle, saved as its key; old saves load as the
  starting hull.
- No change in play. Tests: the base pack loads and is sound; an override replaces, adds and
  renames; unsound content is refused.
