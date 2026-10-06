# The ground's surface fields from the river tiles

- A baked world's river tiles (`rv_*`, beside the 600 m heights) hold its surface fields at half
  their resolution: wetness, scree, bare rock. Read into the same byte-capped cache as the heights
  (`worlds::Heights::surface_at`; a tile's lower block, 513 a side), in the background for drawing.
- The near ground's patches carry them per vertex (`WireModel::data`, a fourth vertex input,
  location 16; patches made before their tiles are read are made again), and the ground's
  material takes them (`GroundIn::wet`, `scree`, `bare`, `surface_on`).
- The engine asks the GPU for its own count of vertex inputs and values between the stages (the
  defaults' 16 are full; every desktop GPU has 28 or more).
- A world's highest ground from its tiles' bounds (`fz.json`), where the bake has them.
