# Hoar E's ground queries no longer stall the flight physics

Playtest over Hoar E: tremor while moving. The sim's flight step took up to 2 s a frame (the crowd threads at
2.5 cores; 66 % of the game's CPU in `LandHeights::hit`): ships near the moved port asked the land
package's ground every substep, and a query scanned a whole cube-face cell of the anchor mesh (about 3,300
triangles, more over a cell's edge) or a 0.05 degree cell of Shasta's 60 m pattern (thousands).

Each anchor tile now has an 8 x 8 sub-grid of its triangles, made the first time it is asked; the pattern's
cells are 0.004 degrees (about 450 m); pyramid tiles are read with the cache unlocked. Release queries: the
port 122 -> 1.5 us, Shasta 145 -> 1.6 us, the anchor mesh 15 -> 1.0 us, the belt's pyramid 0.1 us; every
answer equal to a full scan (`land_heights_query_cost`). A test flight over the port with its traffic holds
119.7 fps.

The sparkling dust over mountains seen from hundreds of kilometres is the renderer's (the app has as much):
cells far finer than a pixel alias as the eye moves. `UNIVERSE_TERRAIN_CELL_PX=1` refines tiles only while
their children's cells stay a pixel across: at 490 km the isolated sparkle pixels go from 2,831 to 29 and the
triangles from 26.3 M to 4.1 M, but distant ridges are visibly softer, so it is off by default
(out/review/e104-game/sparkle/cmp_490.png).
