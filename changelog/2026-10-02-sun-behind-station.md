# The sun hides behind a station

- **The sun's glare** (a HUD overlay) checked only planets and moons for whether the sun was
  hidden, skipping stations, so it shone through a station's main structure.
- It now tests the line of sight against each block of a structure (in its own, turning frame):
  behind any of them, no glare. Checked in a frame from the deck (`platformdeck`).
