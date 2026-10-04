# Sharper ships up close: a tight shadow cascade, compressed textures, tighter packing

- **Shadows: a third cascade round what's looked at.** The near cascade spans about 200 m, so
  a texel was 5 cm: a camera a few metres from a hull saw stair-stepped shadow edges. The
  game now tells the renderer how far off the watched ship is (`Frame::shadow_focus`: the
  observer's ship, ours from the chase camera, the studio's), and a tight cascade 1.2 times
  that wide (6 m at least) is drawn round it: centimetres to millimetres a texel.
- **Model textures block-compressed** where the GPU has BC (wgpu's `TEXTURE_COMPRESSION_BC`;
  desktop GPUs, Windows included): colour and the packed occlusion/roughness/metalness as BC1
  (an eighth of the memory), normal maps as BC5 (a quarter); encoded at load, side by side on
  the CPU (the MC-07's three 8192 maps in about 2.8 s), mips included. Its maps from about
  1 GB of GPU memory to about 150 MB. The shader rebuilds a normal's z from x and y (BC5 keeps
  two channels; exact for unit normals). Uncompressed as before where BC isn't there.
- **Bake packing a texel apart** (was two): 42% of the atlas on the hull (was 35%).
- `mc07.glb` re-exported (8192 px, same pose).
