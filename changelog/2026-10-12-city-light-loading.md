# City lights do not load terrain during drawing

Decorative city lights sample resident visual terrain, matching the nonblocking sky path, instead of invoking the synchronous physical surface sampler. Fine terrain can appear as it becomes available. Physical terrain and collision sampling are unchanged. A separate city-light profiler scope identifies any remaining drawing cost.

Validation: combined loader baseline passes 234 workspace tests, registry build and no-dev check; final near-TRD1 delayed-loading GPU regression is repeated for this change.
