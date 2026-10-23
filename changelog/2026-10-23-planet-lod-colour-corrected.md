# Planet LOD colour, corrected: scene colours are linear; the pool decodes its sRGB; look=raw

eb8d79964 passed the package's sRGB colours to the base tiles raw, after the colour finding read the non-sRGB
swapchain as "colours pass through as authored". That was wrong: the scene is lit in linear light and the blit's film
curve (AgX, or with the tone map off a 2.2 gamma) encodes it for the screen at the end (`blit.wgsl` `film`). The base
tiles' decode was right; the bug was the pool's: its colour bytes are documented sRGB and were unpacked as if linear,
so the pyramid's tiles were lit too bright and pale.

- Base tiles, land layers and the objects' legend decode the package's sRGB again (`package_colour`, `srgb_to_linear`).
- The pool's shader decodes its sRGB colours (`pool_srgb`): the pyramid's tiles now match the base's. The terrain is
  the pool's only user.
- `look=raw` in `UNIVERSE_PLANETLOD_AT`: the tone map off (light × 0.9, gamma 2.2, no AgX, no look), for comparing with
  the lab's views. The "blue haze" measured over the disc was the film curve; the planet LOD app draws no air.
- Kept from eb8d79964: `light_exaggeration` from `info.json` for the base tiles, the view's ambient and sun.

lod_e124s43_v3 from 22,635 km, nadir, the reference sun, disc means: film 11.6, 14.7, 15.0; raw 17.4, 20.7, 21.9.
Light exaggeration on/off (raw): detail 3.44 against 3.02 (stddev of a 3 px high-pass): the base mesh at that height
is coarse, so its relief is its triangles'. The workspace's library tests pass.

Then (the owner, "Left is still looking right"): the planet LOD app draws without the film curve by default (`look=film`
puts it back), so its far view is the lab's look, haze-free; beside the reference prediction, the colours match.
