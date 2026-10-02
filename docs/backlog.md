# Backlog

The game's smaller items, roughly in order (as of 2026-10-02). The world's foundations come first: see `roadmap.md`. Economy tuning waits for them. Small
fixes go straight in; this is for what needs deciding or building.

1. **Hull space balance.** A Drover's modules take 132 m³ of its 5,269 m³
   hull (2.5%): space never limits a build. Count a usable share of the hull
   (structure, wiring, crew space take the rest), make module volumes
   realistic, and let the hold be the space left, so every module added
   takes room from cargo.
2. **Economy tuning** against pacing targets (proposed: 5–10k CR in the
   first hour, first modules by hours 2–4, a mid hull 120–250k at 15–25 h).
   The economy has maker/user prices, population growth, hunger, death,
   migration and shuttles; none of it tuned yet.
3. **Upkeep that consumes goods:** crew life support (food, water a day),
   ammunition restocked from goods, overhauls using metals and machinery,
   works wearing (tools, machinery as spares). Steady demand, reasons to trade.
4. **Demographics page,** varied starting populations (by world size,
   habitability, age), more settled systems than the core 5.
5. **Flight-assist modules:** rate damping, translation assist, as buyable
   steps up from raw manual; nav module subscriptions.
6. **Key scheme regroup:** free letters nearly gone (manual took G, docking O).
7. **Ship models from Blender** (glTF import).
8. **Endgame player outposts:** modular mining bases from blueprints,
   licences in faction space, towing asteroids.
9. **Engine headroom:** 100k ships at 1.5x budget; seed-only checkpoints.
10. **Trademark** FREEFALL (the user's to do).
0. **Visual revamp (mostly done):** `docs/art-direction.md`. Done: render core (HDR, MSAA,
   no outlines), materials and lights, shapes (ships, station), paint and liveries, the HUD
   organised, the radar scope, planets surfaced per pixel, landscapes (quadtree ground on the
   physics terrain, fine grain), the sun's glare, the galaxy map, the gate's push. Left: the
   gate's own redesign, the sprint and interceptor shapes, atmosphere haze and the sky's fade
   with height, the sun's corona, a cockpit frame (optional).
