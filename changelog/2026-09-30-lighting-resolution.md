# 2026-09-30 — Resolution, lighter lines, and sunlight for relief

User: "refine our UI a bit more. Increase resolution, maybe make those lines a bit less sticky.
See if we can give a bit more relief to planet surfaces, also some to the objects like station
and gates, also to the ship itself."

- **Resolution**: the 3D scene renders at 540 lines, up from 270. The HUD stays at 540 (hud_scale
  1, was 2), so text keeps its size. Lines are now half as thick relative to the screen, which is
  the "sticky" look gone, while staying pixel-crisp and retro. GPU cost is unchanged in practice
  (~0.4 ms).
- **Sunlight** (engine): `Frame::light` (the system's star) and flat-shaded draws
  `model_shaded` / `model_colored_shaded`.
  - Each face gets one tone: base × (0.12 ambient + Lambert), with its normal turned away from
    the model's center.
  - Edges dim on the side facing away from the light, down to 30%.
  - The star itself stays unlit (it's the source).
- **What's lit**:
  - planets and moons: terrain globes, with their fill raised from 0.12 to 0.5 of the surface
    colour on the lit side, plus gas giants and plain bodies;
  - the station (grey plating, white edges) and gates (their own colour);
  - the player's ship and settlers (grey hull, coloured edges).

  You get day and night sides, a terminator, and faceted relief on hulls.
- **Ground relief up close**: the local surface grid's faces are shaded by their true slope to
  the star, so hills and valleys read by light. Its lines dim on the night side.
- **Grid lines no longer break into dashes** at grazing angles: the ground fill sits slightly
  below its lines (0.2% of the distance from the camera), locally, instead of raising the global
  line depth bias, which would let hidden hull edges show through.

Honest limits: from orbit, mountains a few km high on a body thousands of km across barely show,
so the terrain isn't exaggerated. Relief shows up close and at low sun angles. A body lit from
directly behind the camera looks flat, as it would.

## Follow-up: grid detail by distance

User: "those grid lines over planets and stars are just too overly sticky and thick, maybe make
grid detalisation based on distance. Distant planets only need a outer round circle."

- A body's latitude/longitude grid now depends on its size on screen (`scene::grid_detail`):
  - none below 80 px of radius: a lit disc and its outline circle only;
  - fading in up to 500 px;
  - never more than half brightness, since the shading carries the shape. Up close the grid
    reads as fine etched lines.
- Stars: outline and halo, with a faint grid only when one fills the view.
- Crater rims show from 150 px (was 40).
- Engine: `model_shaded_faded`, and `model_colored_shaded`'s `line` is now the edge brightness
  (0 = faces only).
