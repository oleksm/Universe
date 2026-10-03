# AgX tone curve; specular anti-aliasing

- **Tone curve: AgX** (Blender's default view since 4.0) replaces the ACES fit: a model lit in
  Blender looks the same here; brights roll off toward white without per-channel hue skews
  (orange paint stays orange in sunlight); space stays black. Exposure 0.5 into it.
- **Specular anti-aliasing** in the PBR shader: where the normal swings across a pixel (fine
  normal-map detail, far away) the roughness widens by the swing (Kaplanyan & Hoffman), so glints
  don't sparkle frame to frame.
- Temporal anti-aliasing waits for foliage (which needs motion vectors anyway): the scene already
  has 4× MSAA on every edge.
