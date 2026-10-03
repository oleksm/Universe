# The galaxy and the charted region

*A world article: where the stars are and why. In code: `crates/world/src/galaxy.rs` (the
region), `crates/world/src/network.rs` (home and the first gates). Kept current as it changes.*

## In one breath

The galaxy is a two-armed spiral about 19,000 ly across, described by **one density field**:
how many stars per cubic light year at any place (a disc, two arms, a bulge), scaled so it's the
**real density near the Sun** where we live. Its stars come from the seed in **sectors** (cubes
100 ly a side), as many as the density says, whenever something looks there: the galaxy map
shows them anywhere, from the whole spiral down to single stars. Play happens in **the charted
region**: the 200 ly cube of sectors round home on an outer arm, 4,000 ly out, where
neighbours are 4-6 ly apart as Alpha Centauri is to the Sun (about 32,000 stars, by the real mix
of star classes: three in four are red dwarfs). More regions, the same sectors made into
systems as they're reached and spread over engine nodes, come later.

## The numbers

| | Kind | Value | Why |
|---|---|---|---|
| Star density | Real | 0.004 per ly³ (0.14 per pc³) | Stars near the Sun: one per about 250 ly³. Main-sequence and giant stars within 20 ly (RECONS census; Gaia's Catalogue of Nearby Stars, Gaia Collaboration 2021, [arXiv:2012.02061](https://arxiv.org/abs/2012.02061)). Brown dwarfs and white dwarfs left out: no systems to visit. |
| Neighbour spacing | Derived | median about 4.5 ly | from the density: the nearest of a random spread at 0.004 per ly³ (`0.55 / n^(1/3)` ≈ 3.5 ly mean nearest; the seeded lanes come out 3.7-7 ly) |
| Class mix | Real (rounded) | M 76.5%, K 12%, G 7.6%, F 3%, A 0.6%, B 0.2%, O 0.003% | the main-sequence stars near the Sun (Harvard spectral classes, by number) |
| Region side | Tuning | 200 ly | **room for decades of play and the star count the engine already handles**: about 32,000 stars (about 1.5 MB), about 40 gate hops across; a 40 ly explorer's epic is a fifth of it. 100 ly would be cramped (4,000 stars), 500 ly needs a spatial index (500,000) |
| Region's place | Tuning | on an outer arm 4,000 ly out from the centre, in the plane | the outer disc, about where the Sun is in ours (the Sun is about 26,000 ly out in a galaxy five times bigger, near an arm) |
| Galaxy shape | Tuning | 40,000 sample stars of a two-armed spiral (pitch 13°) with a bulge, about 19,000 ly across, gathered on a grid (82 ly cells) and softened; thinning above and below the plane (about 150 ly) | the look of the galaxy map (its glow is the same grid); the stars are made by it, scaled so the region averages the real density |
| Sector | Tuning | 100 ly cubes; their count from the density at their middle | small enough that the density hardly changes across one, big enough to be few; a sector's stars are evenly spread, so its first few are a fair sample (the map thins by drawing fewer of each) |

## Home

The first home is a sun-like star (G or K) near the middle of the region, with a station
round an **Earth-like world** (surface gravity under about 1.1 g, so a starter ship lands on it)
and at least four planets. Its four nearest neighbours are linked by gates: a spanning tree plus
up to two loops, every system 1-3 gates (`network::build`). With real spacing the lanes come out
about 4-7 ly, the typical gate of `hyperspace.md`, so a 100 t ship crosses one in under a minute.

## The map

The galaxy map is the whole galaxy from above, one continuous zoom:
- **Stars** from their sectors in a **slab** 40 ly thick round your plane (fading with height
  above or below it; the whole depth flattened would be noise), at most about 30,000 a frame:
  past that every sector is thinned alike, so the arms still show denser.
- **The glow** is the shape's grid itself (`galaxy::shape_grid`): a bright arm is dense with
  stars when you zoom in. It takes over as the stars get too small, with a sprinkle of the
  shape's own sample stars over it.
- It opens at a 5 ly scale bar on your neighbourhood.

## Open questions

1. **More regions:** made from the seed as they're reached, each on an engine node; stars named
   by region and number.
2. **Density across the galaxy:** denser in the arms and towards the bulge, thinner outward;
   one density for the one region today.
3. **Multiple stars:** about half of sun-like stars are in pairs or more; every system is
   single today.
