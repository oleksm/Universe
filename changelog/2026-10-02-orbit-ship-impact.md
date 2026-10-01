# Orbiting a ship; the impact warning

- **Orbiting (or keeping at range from) a ship:** the guide was drawn about the target's last
  radar fix. A fix is a moment old, and at orbital speed (7.8 km/s here) a moment is hundreds of
  metres, by a different amount each frame. The goal jumped by up to ~500 m a frame (distance to
  go 39 → 572 → 398 → 61 m in consecutive frames), with the arches jumping with it. It now uses
  where the ship is drawn, at the moment drawn: distance to go holds at 0 m, and frame-to-frame
  change dropped from ~3,000 to ~150 px. Dev scenarios `orbitship`, `orbitshipwatch`.
- **Impact warning:** the Nav key that switched it was called PROXIMITY. It's **IMPACT WARNING**
  now (still P), and its displays (the IMPACT bracket, the predicted path, the collision line)
  show in Nav mode only, where its switch is. It still defaults off.
