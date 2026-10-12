# Preserve transparent equipment materials in Studio

- Keep glTF opaque/mask/blend settings through the shared equipment projector,
  including clipped triangles and flight transforms.
- Use the shared engine canvas material pass for every Studio view; retain
  authored pane geometry and opaque fallback/structural envelopes.
- Add material propagation regression coverage. Texture sampling, refraction and
  intersection-independent transparency remain outside this change.
