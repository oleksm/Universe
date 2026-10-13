Fix Studio equipment visibility with GPU per-pixel reversed depth and perspective
interpolation. Opaque model triangles bypass font-atlas coverage; catalogue
cameras clear independent depth, UI retains layering, and walk walls share the
same depth buffer. Meshes, placement, saves, collision and motion gates unchanged.
Matched comm-dish captures remove speckled seams and rear-part bleed-through.
135 workspace tests and registry build pass. GPU crossing triangles agree exactly
in opposite submission orders; UI overlay and independent camera tests pass.
Six Vulkan captures inspected; both release binaries rebuilt and Studio launched.
See docs/studio-installed-models.md and docs/diagnostics/studio-depth-v1.json.
