# PGS1 terrain-shadow comparison

Added development-only `UNIVERSE_PGS1_SHADOWS=on|off` (default off) for
canonical local terrain casters, plus exact comparison-time logging and a
canonical low-sun occlusion screening example. Terrain/physics/water unchanged.
Workspace tests, release and player compilation pass. Identical low-sun Vulkan
captures measure the switch but show negligible visual difference; mountain
shadow appearance remains unproven. See docs/pgs1-integration.md for evidence,
reproduction and cascade/LOD limits. No registry install or branch merge.
