# The galaxy and the charted region

*A world article: where the stars are and why. In code: `crates/world/src/galaxy.rs` (the
region), `crates/world/src/network.rs` (home and the first gates). Kept current as it changes.*

## In one breath

The galaxy is a two-armed spiral about 19,000 ly across. Play happens in **the charted region**:
a cube **200 ly a side** in the outer disc, filled with stars at the **real density near the
Sun**, so neighbours are 4-6 ly apart, as Alpha Centauri is to the Sun. Its 32,000 stars are
made from the seed, by the real mix of star classes (three in four are red dwarfs). The galaxy
beyond is a backdrop for now: a glow on the map, no stars. More regions, made from the seed as
they're reached and spread over engine nodes, come later.

## The numbers

| | Kind | Value | Why |
|---|---|---|---|
| Star density | Real | 0.004 per ly³ (0.14 per pc³) | Stars near the Sun: one per about 250 ly³. Main-sequence and giant stars within 20 ly (RECONS census; Gaia's Catalogue of Nearby Stars, Gaia Collaboration 2021, [arXiv:2012.02061](https://arxiv.org/abs/2012.02061)). Brown dwarfs and white dwarfs left out: no systems to visit. |
| Neighbour spacing | Derived | median about 4.5 ly | from the density: the nearest of a random spread at 0.004 per ly³ (`0.55 / n^(1/3)` ≈ 3.5 ly mean nearest; the seeded lanes come out 3.7-7 ly) |
| Class mix | Real (rounded) | M 76.5%, K 12%, G 7.6%, F 3%, A 0.6%, B 0.2%, O 0.003% | the main-sequence stars near the Sun (Harvard spectral classes, by number) |
| Region side | Tuning | 200 ly | **room for decades of play and the star count the engine already handles**: about 32,000 stars (about 1.5 MB), about 40 gate hops across; a 40 ly explorer's epic is a fifth of it. 100 ly would be cramped (4,000 stars), 500 ly needs a spatial index (500,000) |
| Region's place | Tuning | 4,000 ly out from the centre, in the plane | the outer disc, about where the Sun is in ours (the Sun is about 26,000 ly out in a galaxy five times bigger) |
| Galaxy shape (backdrop) | Tuning | two arms, pitch 13°, a bulge, about 19,000 ly across | a familiar spiral for the map's glow; scaled down from the Milky Way's 100,000 ly |

## Home

The first home is a sun-like star (G or K) near the middle of the region, with a station
round an **Earth-like world** (surface gravity under about 1.1 g, so a starter ship lands on it)
and at least four planets. Its four nearest neighbours are linked by gates: a spanning tree plus
up to two loops, every system 1-3 gates (`network::build`). With real spacing the lanes come out
about 4-7 ly, the typical gate of `hyperspace.md`, so a 100 t ship crosses one in under a minute.

## The map

The galaxy map shows the region as seen from above, as a **slab** 40 ly thick round your star
(stars fade with height above or below your plane; drawing the whole 200 ly depth flattened
would be noise). It opens at a 5 ly scale bar on your neighbourhood; zoomed out, the region's
edge and the galaxy's glow.

## Open questions

1. **More regions:** made from the seed as they're reached, each on an engine node; stars named
   by region and number.
2. **Density across the galaxy:** denser in the arms and towards the bulge, thinner outward;
   one density for the one region today.
3. **Multiple stars:** about half of sun-like stars are in pairs or more; every system is
   single today.
