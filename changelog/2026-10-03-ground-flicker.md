# Ground flicker near the surface: fixed

- **The cause:** the ground's patches split into four finer ones only when all four are made;
  a patch unused for 600 frames is dropped. Children out of sight (behind the horizon) were
  never drawn, so never "used": after 600 frames (2-3 s at a high frame rate) they were
  dropped, their parent couldn't split, and for a frame a coarse patch stood in at your feet
  (hundreds of metres across, metres above or below the real ground: the eye under it, the
  sky showing through, the ground cutting the legs, shadows gone). Made again at once, so a
  frame later it was right, then the next ones went: steady a few seconds, then a burst.
- **The fix:** while a patch is split, all four children count as used.
- Found from slow-motion phone video; confirmed by logging stand-ins (none for 600 frames,
  then a cascade every frame, standing still) and gone with the fix (none in 25 s; frames
  past the 600 mark captured steady).
- Dev: `lowflight` takes `UNIVERSE_ALT` (height, m), `UNIVERSE_SPEED` (m/s) and
  `UNIVERSE_CHASE` (chase view).
