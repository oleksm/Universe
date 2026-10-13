# City lights do not load terrain during drawing

Decorative city-light radii are prepared by globe workers using the original physical surface sampler. Drawing reads only prepared radii, preserving the original placement. Lights appear when their globe is ready. The first attempt using resident visual sampling still spent292ms in city lights, so it was superseded by this worker preparation. Physical terrain and collision sampling are unchanged. A separate city-light profiler scope identifies any remaining drawing cost.

Validation: combined loader baseline passes 234 workspace tests, registry build and no-dev check; final near-TRD1 delayed-loading GPU regression is repeated for this change.
