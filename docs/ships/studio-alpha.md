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
