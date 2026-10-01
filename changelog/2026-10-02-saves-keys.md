# Saves by key (C0.4)

- **Version and content:** a save records its format `version` (1) and the hash of the content
  it was made with. Loading a save made with other content says so ("SAVED WITH OTHER CONTENT").
- **Content by key:** the hold is stored by goods key (`goods.food/17`, `ore.stony`) and the hull
  by its key (`hull.drover`), not by position in a list. Every good has a stable key: a generated
  one is its kind and its number among them. Goods the content no longer has are dropped on load.
- **Older saves** (no version, goods by catalogue position, no hull) still load.
- **Fixed:** loading a quicksave dropped the hold and the record of dug-out rocks. The loader's
  record had no fields for them, so they were written but never read back.
- Tests: the round trip keeps the hold and a dug rock, and writes keys; an old-style save loads.
