# Flicker near the ground fixed

- **The ground leapt for a frame** (a flash of sky, sand and sea under the ship on a landing): our
  ship is drawn between the engine's last two views, but not when it "jumped" (a respawn, a
  gate), judged by moving more than 20 km between them. Around a world the ship moves tens of km a
  second with it, so two views a hitch apart (half a second) passed that line: the ship snapped
  to the newer view while the world was drawn at the moment between, hundreds of metres to
  kilometres off, sometimes putting the camera under the ground (backfaces culled: sky, and the
  patch skirts as bands). Now a jump is judged against where the ship's motion would have taken
  it. Checked by stalling the game 0.7 s mid-descent: before, the ship drawn about 300 m off the
  world for four frames (the readout's OFFSET 297 M); now steady.
- **Also: worlds' surface maps never overwrite each other in a frame.** With more worlds drawn
  than map layers, a layer already given out that frame could be reused for another world, so
  one world showed another's ground. The most-drawn worlds (the one underfoot is many patches)
  and those already up keep their layers; the rest go plain that frame. Layers: 16 (were 8).
- Dev: `UNIVERSE_ALT` sets the autoland scenario's stopping altitude.
