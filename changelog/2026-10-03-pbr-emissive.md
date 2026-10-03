# Lamps lit on imported models

- **Untextured emission was off:** a material with an emissive colour but no emissive texture
  was multiplied by a black stand-in texture, so every such lamp, window glow and engine glow
  on an imported model showed nothing. The stand-in is white now, as glTF has it.
- **Emissive strength read:** `KHR_materials_emissive_strength` (Blender exports it for any
  emission over 1): a lamp at strength 40 now shines 40 times its colour, not once.
