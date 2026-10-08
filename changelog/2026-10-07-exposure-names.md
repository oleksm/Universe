# Three exposures named for what they are (audit phase 4, #10)

- `EXPOSURE` meant three things. The eye's adaptation (brightness as irradiance to a power,
  0.45) is `ADAPT` in `frame.rs`, and the shaders get it from Rust in the shared constants block
  (env.wgsl's hand copy gone). The planet's light on a face, adapted on its own curve (0.3), is
  scene.wgsl's `FILL_ADAPT`. The camera's exposure before the tone curve (0.9) stays blit.wgsl's
  `EXPOSURE`. Same numbers; frames of the deck and Heath's range as before.
