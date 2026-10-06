# Captures with the screens asleep

- A capture asked for (`UNIVERSE_SCREENSHOT`) is drawn and saved even when the window gives no
  frame (screens asleep, a window hidden): it comes from the offscreen composite, and only the
  copy to the window is skipped. Before, the frame was dropped and no capture written, silently.
- Seen with it: the lab's ~150 m sample over Heath's range (36.6N 34.6E, evening sun, 1.5 km up)
  against the 600 m ground: the 600 m facets give way to ridges and valleys; the tile edges don't
  show. Frames in the lab's `out/from-integrator/heath_150m_{before,after}.png`.
