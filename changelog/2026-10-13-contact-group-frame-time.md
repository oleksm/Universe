# Contact group: visible-window frame time

- The readiness trace stamps each frame with `wall_s` (wall clock since the trace began).
- It no longer rewrites the whole trace every 120 frames (that cost a frame 35–83 ms late in a long run): frames are
  appended to `<trace>.frames.jsonl`, and `trace.json` is written whole at exit.
- Visible-window walk from crest 1, 140 s, 194 m, 51 groups: frame p95 8.93 ms, max 21.7 ms, none over 33.3 ms
  (docs/diagnostics/contact-group-runtime.md).
