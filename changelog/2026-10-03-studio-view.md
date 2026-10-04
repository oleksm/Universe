# Studio view: a ship lit as a modelling tool lights it

- Watching a ship (ours or any other), **U** (INSPECT IN STUDIO, on the action grid) puts it
  in the studio: alone on a neutral grey backdrop, under studio lights that turn with the eye:
  a key from above and to the left (with its shadows), a broad soft light from the opposite
  side (the renderer's area light, glints included) and a raised ambient floor. U again, or
  focusing anything but a ship, goes back. Orbit and zoom as usual.
- For judging a design against Blender's viewport; flight and everything else stay lit as
  space lights them (one hard sun, planetshine, a dark ambient).
- The renderer's ambient floor is now per frame (`Frame::ambient`, `SHADE_AMBIENT` in the
  world).
- Dev: `UNIVERSE_STUDIO` with the `settlement` scenario.
