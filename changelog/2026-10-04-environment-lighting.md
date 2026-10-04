# Environment lighting (image-based): ships lit by the world round them

- Each frame the GPU draws the environment into two cube maps (`engine::env`,
  `shaders/env.wgsl`), from what the frame knows: the dark sky's glow (the floor the meshes
  take), and the world nearest as a sphere lit by the sun on its day side, in its colour
  (its light taken in as the eye takes the sun's: irradiance to `EXPOSURE`, as the ground is
  drawn). The sun isn't in it: it still lights directly, with its shadows.
  - Specular cube, 128 px, 8 mip levels: each level the environment as a surface of that
    roughness mirrors it (GGX, importance-sampled, 64 samples).
  - Diffuse cube, 16 px: what a matte face turned that way takes in (cosine-weighted, 256).
- Models (glTF) light with them instead of the ambient constant and the planet fill: diffuse
  from the diffuse cube, reflections from the specular cube at the surface's roughness
  (split sum, Karis's fit), both under the baked occlusion. A hull's shaded side now takes
  the world's light, gradients and colour (on Treistun f, its red ground), and glossy parts
  mirror it.
- The studio is an environment too: soft boxes (one on the key light's side, a broad one
  opposite) over a grey cyclorama, instead of a fake planet and a raised ambient.
- Not yet: meshes (stations, gates, buildings) still lit the old way; what the environment
  holds is the world and the sky, not stations or ships nearby.
