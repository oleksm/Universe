# 2026-09-30 — No hang after leaving the window

User: "if I move focus away from the game for too long (maybe 10sec) the game will hang up. Any
chance to be able to get back?"

- **Likely cause** (not reproduced here, since window focus can't be moved from the dev
  environment): the main thread handed each frame to the render thread and waited while one
  was queued. With the window hidden or unfocused, a Wayland compositor can stop handing out
  surfaces, so the render thread waits on it. The queue fills and the main thread blocks too,
  and with it the event loop. A compositor that gets no answer to its pings for about 10 s
  judges the app hung.
- **Fix.** The handover never waits (`render_thread`): a finished frame goes in a one-frame
  slot, replacing one not yet taken, and the render thread draws the newest. A frame carrying
  a screenshot is never replaced (screenshot runs wait for it instead).

## Follow-up: 5000 FPS

User: "5000 FPS is an interesting twist"

The never-waiting handover let the main thread build frames as fast as it could (about 5,000 a
second, counted as FPS), though only one per refresh was shown, and it burned a core. Now, while
a frame is still waiting, the main thread waits for the render thread to take it, at most 50 ms
(`render_thread::PATIENCE`). Normally it's paced by vsync again (about 166 FPS here). With the
window hidden and the surface stalled, it carries on after 50 ms and keeps answering the
compositor.
