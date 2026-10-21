# Hoar E draws e104 with the engine's terrain subsystem

The planet LOD renderer is now the `universe-terrain` crate: `Ground::open` (a planet LOD view or a
`planet-land-pyramid/1` package), `update` for an eye, `draw` placed by a body's transform, `ground_at`
and `drawn_ground_at_world`, and `flight` (the smooth ground reference). `freefall --app planetlod` is a
thin viewer on it (panel, input, pins, boxes): its captures at 10,000, 1,215, 100 and 10 km over belt
170260 and Shasta match the app before the move pixel for pixel.

The game's planet view draws a world whose ground is a land package with it: opened on a thread when
the eye comes within 30 radii, let go past 40 (with its 8 GiB tile cache), placed by the body's centre and
spin turned from the package's frame (the lab's x, y, z is the body's -z, y, x) and scaled from the
package's radius to the body's (Hoar E 6,281.37 km over 6,371 km), in the world's air, casting no shadow
maps as the app draws it. The patches and lines are not used for it.

Landing and contact use `worlds::land::LandHeights` in the world crate: the pyramid's finest covered level
(tile rasters read as asked, 512 kept), else the pattern's finest triangles, else the anchor mesh, as the
renderer's `ground_at`, scaled to the body. The pyramid's file reader moved there too, shared by the
renderer. Under the eye over the belt the drawn and the contact ground differ by 0.5 m.

Needs the registry's Hoar E record (fso e01ef48e1, planet-land-pyramid/1); without it Hoar E keeps its lines
ground. Audit: docs/diagnostics/e104-hoar-terrain-20261021.json.
