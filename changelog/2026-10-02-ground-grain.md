# The ground up close: no boiling, and a grain

- **The cause of the boiling:** where on the world a pixel is was kept in radii, in 32 bits, so it was
  only good to about a metre. With pixels of centimetres under the ship, the map's lookup and the fine
  noise stepped, and slopes taken across the screen spiked at each step (speckle and rings).
- **On a patch, the lookup's gradients come from the eye's metres** (exact); the coarse slope shading
  fades out once a pixel is under a few metres (the mesh's own shape shades it there).
- **The ground's fine grain:** each patch passes its origin (m, the world's frame) wrapped to 4096 m,
  and the shader adds the vertex's metres. Noise on a lattice that wraps with it matches across
  patches and is exact to the millimetre. Its octaves run from 128 m down to half a metre (pebbles,
  ripples, hummocks), the same slope at each scale, fading in as they grow to a few pixels. They go into
  the shading heights and a touch of colour. (It rides in the instance columns' spare w: the vertex
  attributes are all used.)
