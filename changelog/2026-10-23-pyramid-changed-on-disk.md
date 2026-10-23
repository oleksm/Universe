# Land pyramid rebuilt under a running app: one clear stop

Planets `20261023T174347-planets-af51`: forge_e124s43-mesh/pyramid was rebuilt while the planet LOD ran; it kept the old
`pyramid.json`'s offsets, every new tile failed ("a vertex outside the tile") and it drew 4 fallback tiles, silently.

- The pyramid keeps its `pyramid.json`'s stamp (modified time, length) from when it was opened and looks again about
  once a second (`STAMP_EVERY` updates). Changed: one error in the log naming the folder, no more tiles queued
  (they'd be read at the old offsets), what's loaded still drawn; `PyramidStats::stale`, and the planet LOD panel's
  row "CHANGED ON DISK — reopen". Opened again, it's read anew.
- Reloading in place was not taken: the index, the tiles cached and the loaders' shared source would all have to be
  swapped together under the loader threads.

Test: `pyramid_changed_on_disk_stops_loading_and_says_so`; the workspace's library tests pass.
