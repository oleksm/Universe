# Terrain tiles stream where the eye looks and heads, and go when left behind

The owner, flying forge_e104-fine low over the belts: the land pyramid lagged; its cache sat full at 8 GiB
and only let tiles go under pressure. Three causes, three changes (`crates/terrain/src/pyramid.rs`):

- Loading: one loader built one working set at a time and the next was only asked for once it was done, so
  a fast eye waited behind tiles it had passed (pending to 799). Four loaders now share one plan rewritten
  every update: the tile underfoot, the view's coarse before fine and each level by where the eye looks and
  heads (nearest, toward the view's middle or the way it moves, first), then what it will select
  LOOKAHEAD_S = 2 s ahead at its speed and heading. Tiles no longer wanted are never started.
- Keeping, by place (the owner's rule): a tile unused for a moment goes, whatever the budget, once it is
  behind the eye (behind where it looks and where it heads) past twice the distance it refines at, or anywhere
  past six times it. Over budget, the most remote go first (behind and far before ahead and near).
- The GPU mesh pool's one buffer was capped at 1 GB; a fine package's cover at low altitude needs more (the
  pool's working set full on every second of the owner's flight). It now grows as needed up to the adapter's
  limit, at most just under 4 GB (its 32-bit word addresses and one binding).

Frozen copy of forge_e104-fine, 60 s of the diagnostic pan at 1.5 km (to 11 km/s): tiles drawn of those
selected 75 % -> 99.8 % (the worst second 52 % -> 98 %), pending 94 -> 1 on average (799 -> 13 at most),
fallback tiles 11.8 -> 0.1, pool-full warnings 39 -> 0, 119.7 fps (the slowest second 74 -> 109). Repeated zoom
(10 km - 6,300 km - 350 km): every hold full, no pool warnings; zooming back in now loads the fine tiles
again (left far behind, they went), a second or two of transition the old cache kept warm.
