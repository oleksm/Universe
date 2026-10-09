# The renderer's frame by pass, its setup by part (audit phase 5)

- `Renderer::render` reads as the frame's order: sun probe and cascades set up, then
  `shadow_passes`, `scene_pass`, `front_pass` and `hud_pass` (each its own method, with what it
  depends on as arguments: whether there's a light, the clear colour, whether the probe wants the
  depth), the composite and the upscale. 291 lines to 182.
- `Renderer::new` hands three self-contained parts to their own constructors: `Globes::new` (the
  globe maps and colours, their views and sampler), `hud_atlas` (the font's atlas bound and the
  HUD's triangle pipeline) and `blit_stage` (the last stage's layout, its window and capture
  pipelines and samplers). 505 lines to 361. The shadow bind group stays where the shadow map,
  the globes and the environment meet.
- Frames of the deck and Heath's range as before.
