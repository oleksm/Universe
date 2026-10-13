# Studio shares the engine scene and an isolated live World

Studio DESIGN, catalogue, WALK, BALANCE and DRIVE now submit retained engine scenes. Installed equipment retains the original leased PBR model and uses the engine's material, depth, transparency, upload and viewport paths. The old Studio triangle projector/sorter and its installed-model face copies are removed. Flat authoring views use real orthographic projection; module selection uses the engine scene picker.

Editor decks and walking sessions belong to the client session. WALK uses the normal world Walker/colliders, including the selected native hull. DRIVE compiles an owned uncommissioned ClassSpec and runs the normal World ship step in a minimal isolated system, with the engine TickClock. Shared fan/battery/landing devices apply real resource limits and takeoff rules. Unsupported fuel combinations or missing physical device data prevent flight with an explicit diagnostic. Geometry preparation runs on a revision-bound worker while the editor remains cancelable.

Blueprint annotations, authoring tools, budgets and YAML editing remain client features. Draft collision strength is explicitly unrated; this work does not certify an authored frame or introduce individual landing spring mechanics.

GPU validation also found a shared device performance regression: assisted ticks recomputed thrust authority even with no translational demand. The world device implementation now skips unused solves and caches exact identical physical inputs; no density rounding or alternate integration is used. Engine geometry counters and GPU timings now aggregate every retained viewport, PBR instance and ordered HUD span.
