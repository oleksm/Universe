# Studio transparent equipment

Studio keeps the installed glTF material's `blend` flag and optional alpha cutoff
on each shared equipment face and projected triangle. Base-colour alpha is not
used to infer the mode: glTF OPAQUE ignores it, MASK tests its cutoff, and BLEND
uses it for compositing. Placement, flight transforms and near-plane clipping
preserve these settings. Generated fallback envelopes and structural faces are
explicitly opaque.

DESIGN, the catalogue card, walk/PREVIEW, BALANCE and TEST DRIVE use the same
`Projected::draw` submission. Transparent ordering belongs to the engine canvas,
across all objects in that canvas, rather than a per-object Studio sort. Opaque
and surviving masked fragments write depth; blended fragments test depth but do
not write it and are submitted far to near. Separate catalogue cameras retain
separate depth contexts. UI submitted before a scene's first geometry stays
beneath it; interleaved labels and annotations become overlays after all geometry
in that scene. This keeps labels out of the transparent depth ordering.

This fixes material mode/depth handling, not full PBR shading. Studio's existing
flat-colour path does not sample glTF colour/alpha textures, provide refraction,
or solve intersecting transparent triangles. The window's physical panes and
single-sided closed geometry are unchanged. Collision and physics are unchanged.

The material propagation regression exercises all three modes, low base alpha,
near-plane clipping and the combination of placement with a flight parent.
GPU acceptance must include opaque geometry submitted after glass, reverse
object order, front/back and oblique viewport views, and independent UI/camera
boundaries. Validation results will be recorded with the renderer handoff.

Validation: renderer `a48f7870` plus consumer `c9bc0f51`/`e16c74e6`, combined
at `75f2b4d4`, passes the registry build and all 165 workspace tests. Engine's
33 Vulkan captures cover the prototype and final pane assets, both source and
sorted ordering, and an explicit alpha/UI/depth test. Ships independently checked
eight source/sorted image pairs (all pixel-identical) and inspected front and
oblique pane images: the background remains visible and the frame occludes it.
Results are recorded in `docs/diagnostics/studio-alpha-v1.json`.

Evidence: `/home/alexm/git/universe/out/review/viewport-s2-gpu-fixed/review.md`
and `../viewport-s2-gpu-final/review.md`. These are isolated actual-renderer and
shared-projector tests, not full saved-ship UI/placement acceptance. Main and the
running Studio were not changed by this handoff.
