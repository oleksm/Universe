# Installed equipment available on main

Merged registry cutoff `16c9cc35` and the shared Studio alpha consumer/renderer through `77a17df3`. This adds 49 equipment model references compared with `dda15848`, including viewport S2, airlocks, cargo modules, engines, tanks and life support. Registered models use the existing shared Studio visual path in DESIGN, preview, balance and test drive; operational motion and hull integration acceptance gates remain unchanged.

Installation updates the asset store and `fso`; it does not automatically merge or rebuild `main`. Restart Studio after rebuilding to see this snapshot. Run `./target/release/freefall-studio` from the repository.

The four saved experimental designs currently use 52 equipment types, of which 30 have registered models at this cutoff. The remaining 22 continue in the prioritized asset queue. Evidence for this merge is in `out/review/assets-main-merge/`.

Validation: registry build passed; 165 workspace tests passed; both release binaries rebuilt; 94 registered manifest hashes verified locally. Vulkan DESIGN capture of an isolated commuter-1 copy passed and was visually inspected.
