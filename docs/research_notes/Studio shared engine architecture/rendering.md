# Rendering, materials, cameras, assets and scene sharing

## 1. Which rendering behaviors are actually shared at snapshot 31f68543?

### Takeaway

**Existing:** Studio is already a small `Game` host, but its DESIGN/WALK/BALANCE/DRIVE/catalogue visuals run through a CPU projector and depth-aware HUD canvas, not live flight's resident PBR scene path. Shared asset preparation and shared Studio projection are useful primitives; neither constitutes shared end-to-end rendering. The immediate architectural target is an engine scene viewport, consumed by both flight and Studio, with a shared ship/equipment scene adapter above it.

Scope: read-only audit of checkout HEAD `31f6854326e1a8e78f71cab47c9ccaee7b22379e`; no builds, implementation changes, extra agents or mail. Source links below point into that checkout; the timing report is explicitly separate prior evidence. No external claims or benchmarks were introduced.

### Cited Findings

| Existing entry / mode | Actual branch and rendering | Evidence |
|---|---|---|
| Standalone Studio | `StudioOnly` implements engine `Game`; maintains common assets, delegates input to Interior, returns **default engine camera**, draws full-screen HUD background then Interior. Studio cameras are elsewhere. | [studio_only.rs:68–110][host] |
| Embedded shipyard | Page::Interior calls the same Interior; Page::Layout calls a separate `studio.rs` plan/side/end drawing application. | [shipyard.rs:233–252][yard] |
| DESIGN | Interior dispatches BALANCE, DRIVE, WALK first; otherwise design. It **recreates placements per draw** and calls `draw_design_models`, which opens a canvas and uses `equipment_visual::draw`. Hull/structure/editor guides are separate HUD drawing surrounding this call. | [interior.rs:7748–7764][dispatch], [2731–2743][design], [8186–8215][design-submit] |
| WALK | `Stand` retains procedural faces, collision mesh, equipment placements, members and labels. `draw_stand` builds a new mixed list, projects walls and equipment, depth-sorts **all** items, then emits canvas triangles and HUD overlays. | [interior.rs:4273–4288][stand], [4394–4517][walk] |
| BALANCE | Own orbit camera/projection and HUD grid/arrows; equipment and procedural structure use CPU projector under translation `-visual_origin`. | [test_drive.rs:558–599][balance] |
| DRIVE | Own chase camera/projection, CPU-clipped HUD floor; equipment/structure use `pose * translation(-visual_origin)` through the same CPU projector. The comment “Its shadow: straight down” denotes a line to the floor, not a shadow map. | [test_drive.rs:668–740][drive-camera], [784–793][drive-submit] |
| Catalogue | Priority-0 asset request, fresh neutral placement, separate camera aimed at a small screen rectangle, new canvas scope, CPU projection. This call draws a border but supplies no viewport/scissor clipping contract. | [interior.rs:6078–6093][catalogue] |
| Live flight | Main App draws `scene` then HUD. Native hull and mounted equipment submit PBR draws; close interior decks/walls/hatches use engine `Mesh`. Hull ramp is a separately transformed PBR part. Missing native model uses procedural painted hull/detail. | [lib.rs:1661–1666][app], [scene.rs:959–1008][flight] |
| Observer “studio” | A distinct **existing PBR** studio: sets key light, reflector, ambient, studio environment, backdrop, then calls the **same hull function as flight**. It is not the Interior DESIGN/WALK application. This is the best current proof that engine lighting can serve a studio. | [scene.rs:20–24][scene-entry], [191–211][observer] |
| 2D layout Studio | Plan/side/end regions, custom `to_screen`/`to_plan`, clipped HUD lines. Multiple drawing regions exist here, not multiple PBR scene views. | [studio.rs:954–988][layout] |

- **Shared preparation, two retained representations:** `visual_assets::Visual` contains `PbrModel`, `Arc<Vec<Face>>`, nodes and bounds. Registry preparation always requests faces; it decodes the PBR model then copies indexed triangles into a CPU Face representation. Retained byte accounting includes both. Native model preparation uses the same cache service with path/part identity. This is not two independent file decoders per Studio/flight draw, but it is duplicated retained render geometry for registry assets. — [visual_assets.rs:15–28][asset-shape], [64–99][asset-id], [126–180][asset-prepare], [203–213][asset-bytes]
- **CPU Face deliberately loses material fidelity:** only positions, normals, base factor color, sidedness, blend and cutoff survive. There are no UVs, sampled base texture/texture alpha, metallic/roughness, normal/AO/emissive maps. Every frame projection computes a camera-relative directional diffuse shade `0.3 + 0.7 * max(dot(n, light), 0)`. Merely sharing glTF alpha-mode flags is not equivalent to sharing textured material behavior. — [equipment_visual.rs:12–19][face], [149–232][projector], [visual_assets.rs:166–180][asset-faces]
- **Live PBR is richer:** importer records factors and texture slots for base/MR/normal/emissive/AO, alpha policy and sidedness; GPU shader samples them, discards masks, evaluates lighting/reflection/emission. Graphics toggles control these features. Scene supplies star/eclipsers/reflector/shadows; observer substitutes a studio environment. Canvas triangles do not execute this PBR shader. — [pbr.rs:156–175][pbr-material-import], [shaders/pbr.wgsl:150–222][pbr-shader], [frame.rs:71–95][graphics], [scene.rs:35–56][flight-light], [scene.rs:191–206][observer]
- **Transparency differs in both fidelity and ordering:** canvas preserves OPAQUE/MASK/BLEND, depth-tests geometry, writes opaque/mask depth, sorts blend triangles per canvas. Interleaved UI is moved after the geometry/alpha pass and has no scene depth test. PBR has sided/unsided opaque and blend pipelines and blend depth-write disabled, but `PbrRenderer::draw` iterates pipeline → submitted instance → primitive: **no depth sorting in this path**. Thus replacing canvas with current PBR can improve material fidelity while regressing ordering of overlapping glass; do not promise pixel parity without addressing this. — [frame.rs:791–816][canvas-api], [renderer.rs:1226–1255][hud-pass], [1630–1664][canvas-batches], [pbr.rs:683–717][pbr-pipes], [1188–1208][pbr-draw]
- **Shadows are not a solved parity contract:** Studio equipment canvas casts/receives no PBR shadows. PBR draws opaque/mask casters, excludes blend; its shadow pipeline has no fragment stage or material bind group, and shadow shader only transforms positions. Masked texture holes therefore cannot cut holes in this shadow pass. The observer studio already exercises engine shadows. — [pbr.rs:719–750][shadow-pipe], [1214–1232][shadow-draw], [shaders/pbr_shadow.wgsl:1–20][shadow-shader], [observer][observer]
- **Shared placement differs from mounted semantics:** Studio aligns authored geometry about decoded geometry center (`at - rotation * centre`) in metres; thrust-derived rotation maps +Y to thrust. World free-standing visuals use package bounds center or foot. Mounted equipment instead uses hull mount × equipment primary frame, rejects nonrigid frames and motion packages, subtracts hull COM exactly once. These are legitimate placement modes requiring an explicit shared contract, not interchangeable matrices. — [equipment_visual.rs:47–68][placement], [visuals.rs:18–43][world-placement], [mounted.rs:76–94][mounted-policy], [110–122][mounted-draw], [154–183][mounted-compose]

### Inferences

- **Proposed boundary:** engine owns generic scene/rendering/assets/view contracts; shared game or ship-runtime module owns registry visual resolution, ship graph assembly, mounting/COM and motion semantics. Studio supplies a small editable graph and editor policy. Moving registry-specific logic directly into the low-level engine is not required for “shared engine modules,” and could create an engine→game dependency through today's `Visual.faces: crate::equipment_visual::Face`. — Basis: [asset-shape][asset-shape], [mounted-compose][mounted-compose], [host][host]
- The phrase “shared renderer across Studio” is defensible only for the common CPU projector/canvas path. It does **not** establish parity with live flight. All share the final engine GPU/window host, but different draw representations, shaders, cameras and material semantics. — Basis: [design][design], [drive-submit][drive-submit], [flight][flight], [pbr-draw][pbr-draw]

### Gaps

- Runtime appearance and cost of this exact checkout were not measured. Named shader/material support is not evidence that every asset renders correctly.
- Complete articulated equipment consumption remains absent in the audited mounted path: it explicitly refuses `has_motion`; this research does not establish a complete simulation/animation contract elsewhere. — [mounted.rs:81–87][mounted-policy]

## 2. What CPU work, readiness and coordinate behavior would the migration remove or preserve?

### Takeaway

**Existing:** Studio repeats triangle transformation, normal lighting, clipping, screen projection and canvas vertex uploads every frame. PBR retains vertex/index/texture resources and streams instances, but still performs CPU submission/cache bookkeeping and lacks a generic visibility/LOD selection layer in the audited submission path. Readiness must be modeled explicitly: frozen placement snapshots are not frozen cache lookups.

### Cited Findings

- **Per-frame CPU work:** DESIGN allocates placement list and resolves visuals; catalogue requests and resolves again for the current card. All Studio equipment paths transform each face's positions/normals, backface-test, calculate view coordinates and shade, near-clip, project, and emit three HUD vertices per resulting triangle. WALK adds fresh polygon/projected point vectors, copied label strings and the mixed stable sort. DRIVE builds/clips floor tiles independently. All canvas/HUD vertices upload each frame. — [design-submit][design-submit], [catalogue][catalogue], [projector][projector], [walk][walk], [drive-camera][drive-camera], [renderer.rs:1024–1036][uploads]
- **Repeated sorting is real:** WALK sorts opaque, masked, blended and UI items together; engine canvas batching separately partitions and depth-sorts blends. Opaque sorting is redundant for depth visibility, although equal-depth ties and relative label/member UI order need deliberate migration. WALK comments suggesting wall-occluded labels do not match engine batching: the emitted labels are UI after all that canvas geometry. — [interior.rs:4478–4516][walk-labels], [renderer.rs:1630–1664][canvas-batches]
- **Prior measured evidence only:** `c9b2089c`, runtime stated same as baseline, warm commuter-3 fixture, RTX 5090 Vulkan, 60-FPS cap, another game process running. Approximately 176k WALK triangles: whole WALK 15.857 ms mean; mixed sort 8.642; projection/list insertion 4.833; submission/list consumption 2.344. Observed facing median 54.1 FPS; the owner's exact 30 FPS was **not reproduced**. Timings overlap/nest; do not add whole draw, WALK and render-thread times. These numbers justify investigating sorting/projection, not any predicted PBR speedup. — [prior investigation:3–35][perf]
- **Resident PBR is not zero-CPU rendering:** cache hit skips geometry upload; GPU vertex/index buffers persist. Per frame it constructs a visible-model set, checks/maintains resources, collects/uploads instance records and creates draw/caster lists; GPU draws indexed primitives. Submission computes camera-relative placement and lighting per instance, not per triangle. — [pbr.rs:812–819][gpu-visible], [896–1019][gpu-upload], [1139–1208][gpu-model], [frame.rs:706–745][instance]
- **Two readiness stages already exist:** CPU common cache returns `Option<Arc<Visual>>`; then PBR texture preparation and bounded GPU transfer run separately. PBR substitutes a neutral envelope until ready and retains previous resources through completion tracking. CPU-ready does not mean GPU-ready. Current public submission does not expose an editor-friendly per-object readiness result. — [visual_assets.rs:64–90][asset-id], [pbr.rs:812–844][gpu-visible], [917–990][gpu-placeholder]
- **Pending Studio placement actively promotes:** `Placed` saves fallback faces and `pending=(key, at, rotation)`. On every `project_each`, pending items call `get(key)` again and use the ready visual for that draw. The retained `Placed` is not mutated; a formerly pending item keeps polling even after successful draws and borrows the temporary `Arc` for that projection. A placement constructed ready pins an Arc and never enters that pending lookup branch. The test explicitly says promotion does not mutate saved placement/physics. Do not diagnose WALK/DRIVE as permanently retaining fallback solely because `installed` stays false. — [equipment_visual.rs:25–42][placed-shape], [56–93][placement], [159–170][pending], [432–434][pending-test]
- **Lease nuance:** ready placements retain full cache ownership; pending successful draws hold a temporary Arc, not a persistent promoted lease. Live model submissions attach the `Visual` Arc to `PbrModel`, whose lease mechanism is designed to survive frame/worker clones. Repeated pending requests are current behavior, not evidence of redecoding every frame; actual residency depends on the shared cache. — [equipment_visual.rs:29–31][placed-shape], [pending][pending], [models.rs:182–184][native], [pbr.rs:121–127][lease]
- **Mounts actively re-resolve too:** `mounted::for_ship` creates current draws every frame, polls common hull/equipment services and current fit, and warns/rejects unsupported contracts; it is not a stale frozen mounted graph. Replacing it with retained scene nodes needs fit/version invalidation and pending retry semantics. Native path identity explicitly assumes files immutable within process, whereas registry package identity includes manifest hash. “Engine changes automatically apply” does not currently mean arbitrary hot reload of native asset bytes. — [mounted.rs:127–168][mounted-current], [visual_assets.rs:64–99][asset-id]
- **Culling/LOD bounds:** CPU projector iterates every placed face before only backface/near rejection; it has no object side-frustum rejection, wall occlusion or LOD choice. PBR submission appends every requested model; renderer's `visible` set means submitted model IDs, not geometrically tested visibility. Caster distance is filtered, and GPU face culling exists, but the audited PBR path has no general scene bounds/frustum/occlusion/LOD selection. World `Visuals::spatial` supplies predictive demand, not an automatic PBR mesh simplifier. Scope this negative finding to these paths, not all terrain/rock systems. — [projector][projector], [instance][instance], [gpu-visible][gpu-visible], [gpu-upload][gpu-upload], [pbr-pipes][pbr-pipes], [visuals.rs:89–104][prediction]
- **Coordinate/precision mismatch:** engine camera is f64 position, quaternion orientation, local -Z forward, +Y up, reversed-Z infinite perspective; f64 subtraction occurs before f32 conversion. Studio camera uses f32 basis vectors, positive forward-distance and pixel focal length. WALK near=.08; DESIGN/catalogue=.1; BALANCE/DRIVE=.2 (DRIVE floor=.3). DRIVE casts world craft position to f32 before camera placement. DESIGN “flat” view is long-lens perspective (distance and focal ×25), not orthographic. Engine current Camera has no orthographic projection enum or screen-ray method. — [camera.rs:4–61][camera], [interior.rs:4399–4411][walk-camera], [4538–4578][editor-camera], [balance][balance], [drive-camera][drive-camera], [drive-submit][drive-submit]

### Inferences

- **Proposed staged performance cleanup:** while building the viewport, remove the redundant WALK geometry sort with explicit UI ordering; then conservative per-object bounds rejection. Final resident mesh migration removes per-face CPU work and duplicate Face storage. No quantitative speedup is claimed. — Basis: [perf][perf], [projector][projector], [canvas-batches][canvas-batches]
- **Proposed readiness contract:** persistent asset handle with explicit `Missing / Queued / CpuReady / GpuPreparing / Ready / Rejected` and diagnostic reason; separate representation acceptance (`Neutral`, `MountedRigid`, future articulated), residency lease and immutable content revision. Draw/picking/bounds must select the same effective fallback or ready representation. Promote rendering without rebuilding physics. — Basis: [pending][pending], [gpu-placeholder][gpu-placeholder], [mounted-policy][mounted-policy], [asset-id][asset-id]

### Gaps

- No new measurements distinguish pending lookup overhead from retained placements, quantify duplicate memory, or measure GPU shadow/texture cost for this fixture. Avoid attributing the prior warm WALK cost to loading.
- No runtime parity was established for nonuniform scaling, mirrored transforms or large-distance DRIVE. Current Studio placements are rigid; a future generic transform contract must specify normal matrices/handedness rather than inherit the CPU projector's simple vector transform. — [projector][projector], [placement][placement]

## 3. What must the universal viewport provide, and what should be migrated/deleted?

### Takeaway

**Existing PBR is a reusable backend, not a drop-in universal viewport.** Single-view camera/light globals, render targets, PBR draw state and output composition must become per-view while GPU resource residency remains shared. Prioritize the viewport/scene contract, asset/material parity and explicit overlays; then delete Studio rendering copies. Do not build a second editor-only engine.

### Cited Findings

- **Single-view coupling:** Frame has one camera/light/environment, scene/HUD sizes and PBR list. Renderer uses one target and one globals block derived from that camera; it uploads PBR once, renders scene, front, HUD and composite. `model_pbr_part` has neither view ID nor object/pick ID. Mutating `frame.camera` between model submissions is unsafe as a multi-view strategy: instance positions are camera-relative at submission, but rendering later uses the one final camera matrix. — [frame.rs:99–173][frame], [706–745][instance], [renderer.rs:991–1036][globals], [1065–1082][passes]
- **Canvas scopes are not PBR viewports:** `canvas_scene` increments a depth scope; renderer clears HUD depth per section, keeping color and ordering UI. It does not carry a PBR camera, scissor rectangle, environment or independent scene target. Catalogue currently fits by pixel projection without a viewport boundary. — [canvas-api][canvas-api], [hud-pass][hud-pass], [catalogue][catalogue]
- **Useful overlay primitives exist:** engine has 3D lines/points, screen projection, HUD text/lines and front/scene helpers. However scene depth and HUD canvas depth are separate, HUD doesn't test scene depth, PBR submission always appends to `pbr` (does not branch on `in_front`), and scene depth is normally discarded unless the sun probe requests it. These primitives do not automatically deliver depth-aware editor labels or outlines. — [frame.rs:580–608][overlay-primitives], [423–437][front], [706–708][instance], [renderer.rs:1167–1188][scene-pass], [hud-pass][hud-pass]
- **Picking is app logic:** Interior ray derives from its custom focal/basis camera; hover prioritizes projected points/nodes/lines/member proximity then nearest module envelope/deck ray hits. Current PBR primitive/instance draw records have no selectable ID and current engine camera only projects perspective. Replacing this with mesh picking alone would change intentional editor selection semantics. — [interior.rs:4568–4578][editor-ray], [6537–6577][picking], [frame.rs:192–200][pbr-record], [camera][camera]
- **Scene assembly is also duplicated:** WALK compiles walls/floors and physical stand, BALANCE/DRIVE convert stand structure into CPU faces, while flight calls cached Mesh adapters for interior walls/decks/hatches and separately animates hatch/ramp placement. Reusing `PbrModel` alone leaves these differences. — [interior.rs:4177–4195][stand-build], [5122–5129][craft-build], [scene.rs:967–1003][flight-interior], [models.rs:52][walls]

### Inferences

**Proposed contracts (not existing APIs):**

1. **`SceneSnapshot` / stable `ObjectId`:** instance graph with parent, rigid/defined-scale local transform, visual handle + revision, part/node reference, local bounds, layer/visibility, pick proxy, fallback policy and shadow policy. Shared assembly produces it from authored ship/fit/decks; Studio edits that graph and chooses mode, flight attaches world pose/simulation state. Immutable meshes rebuild on geometry revision, not draw. Preserve independent physical envelopes. — Basis: [stand][stand], [craft-build][craft-build], [mounted-current][mounted-current], [flight-interior][flight-interior]
2. **`ViewportDesc` / `ViewId`:** scene reference, rectangle/clip, physical pixel and logical UI size, camera with `Perspective | Orthographic`, near/far conventions, origin, background, shared render settings, light/environment preset, per-view layer filter and overlay list. Engine owns screen↔ray/project transforms. Catalogue is a viewport with a one-object graph; DESIGN/flat/WALK/DRIVE/BALANCE are camera/controller/policy changes over a shared graph. Long-lens compatibility can be offered during migration; true orthographic is explicit. — Basis: [frame][frame], [camera][camera], [catalogue][catalogue], [editor-camera][editor-camera]
3. **`RenderDeviceResources` + per-view render state:** share PBR models, buffers, texture preparation and upload budgets across all views; retain distinct globals, instance ranges, depth/color targets or atlas regions, transparent queues and shadow state. Gather union asset demand before eviction so a catalogue viewport cannot evict the main viewport's models. Current PBR upload overwrites `self.draws`/instances and computes visible IDs per call; simply calling it once per viewport is not a sufficient shared-resource scheduler. Distinct globals must remain valid through encoding/submission, not be overwritten in one uniform slot. — Basis: [gpu-upload][gpu-upload], [gpu-visible][gpu-visible], [globals][globals], [scene-pass][scene-pass]
4. **`OverlayDepth = SceneTested | XRay | AlwaysOnTop`:** unified world-space guides, labels, wire/envelope outlines and selection highlights; explicit layer/order and occlusion semantics. Renderer retains/resolves depth if needed. Keep menu/widget text as normal HUD. Flat layout grids can remain lightweight editor primitives using shared projection; “100% shared” need not mean rasterizing text or every guide as PBR. — Basis: [canvas-batches][canvas-batches], [overlay-primitives][overlay-primitives], [picking][picking]
5. **`PickQuery(view, pointer, filters)` → stable IDs + distance:** shared view ray + bounds/proxy queries first; optional GPU ID/mesh hit for visual-accurate selection later. Specify pick layer priority, hidden/ghost objects, ready-versus-fallback geometry, viewport clipping and orthographic ray origins. Preserve point/node/member precedence as editor policy. — Basis: [picking][picking], [pbr-record][pbr-record], [pending][pending]
6. **`Material/LightingProfile` and visibility policy:** all modes use the same material data/shaders, with explicit studio/world/unlit/technical presets. Close PBR blend-order and mask-shadow gaps once in engine. Add shared object bounds/frustum and screen-size LOD selection; expose no-LOD/full-detail as deliberate editor policy. Advanced occlusion is a later feature, not a prerequisite claim. — Basis: [pbr-material-import][pbr-material-import], [observer][observer], [pbr-draw][pbr-draw], [shadow-pipe][shadow-pipe], [gpu-visible][gpu-visible]
7. **`PlacementSpace` / asset state contract:** named authored-metre, bounds-centered, grounded, mount-frame and COM-relative placement operations; camera-relative conversion only at view submission. Common asset revision/readiness contract from section 2. Predictive demand must be per-view (current `visuals::VIEW` is one global camera sample), merged by asset key, not reset by switching catalogue/flight cameras. — Basis: [placement][placement], [world-placement][world-placement], [mounted-draw][mounted-draw], [visuals.rs:46–86][view-demand], [instance][instance]

**High-priority migration/deletion sequence:**

| Priority | Concrete change / deletion target | Completion condition (proposed, not run) |
|---|---|---|
| P0 | Engine viewport/camera/resource split, shared graph adapters; keep `StudioOnly` as host. | Main view and catalogue PBR draw concurrently with independent camera, rect/depth/light, shared resource identity and one upload scheduler. Grounded in [host][host], [globals][globals], [gpu-upload][gpu-upload]. |
| P0 | Convert equipment and procedural structure to resident PBR/Mesh instances; retire `equipment_visual::{Face,Camera,Projected,project_each,draw}` and `Visual.faces` extraction/accounting **after consumers migrate**. Keep resolver/placement policy in shared runtime. | All six requested modes plus flight use common submission/material behavior; cold→ready transitions and world/Studio leases agree. Targets: [face][face], [projector][projector], [asset-faces][asset-faces], [asset-bytes][asset-bytes]. |
| P0 | Remove `draw_stand` mixed Item geometry list/sort, wall projection/clipping, and duplicate BALANCE/DRIVE equipment projection; render structure once per revision. | Opaque/mask depth, glass order, equal-depth surfaces and explicit label/member semantics checked. Targets: [walk][walk], [balance][balance], [drive-submit][drive-submit]. |
| P1 | Retire Interior custom camera/ray, WALK basis math, BALANCE/DRIVE project closures and catalogue camera adaptation. | Shared perspective/orthographic pixel/ray roundtrip and per-view input routing; COM subtracted once; large-position precision maintained. Targets: [editor-camera][editor-camera], [walk-camera][walk-camera], [drive-camera][drive-camera], [catalogue][catalogue]. |
| P1 | Move structure/hatch/ramp and mounted assembly into shared scene producer; remove equivalent editor-only visual assembly branches, not collision data. | Same neutral/current pose renders across mode changes, unsupported motion remains explicit. Targets: [stand-build][stand-build], [craft-build][craft-build], [flight-interior][flight-interior], [mounted-policy][mounted-policy]. |
| P1 | Fix engine blend ordering/masked shadows and explicit overlay/pick support. | Overlapping glass, alpha-cutout grilles, transparent/opaque overlap, scene-tested labels, hidden layers and selection priority validated. Targets: [pbr-draw][pbr-draw], [shadow-pipe][shadow-pipe], [picking][picking]. |
| P2 | Shared bounds/LOD/demand; revise single global view prediction. Retain canvas for actual 2D HUD, do not delete it globally. | Offscreen object rejection does not lose shadow casters/picking or cancel another view's demand. Targets: [projector][projector], [gpu-visible][gpu-visible], [view-demand][view-demand]. |

“Automatically gets engine changes” should mean both clients consume the same shipped implementations and contracts, verified by common fixtures; it does not mean zero adapters, identical lighting presets, or native-file hot reload already exists. This architecture can eliminate duplicated rendering ownership while leaving editing tools, authoring graph and mode UI in Studio. — Basis: [asset-id][asset-id], [observer][observer], [host][host]

### Gaps

- Choosing atlas/scissored scene regions versus separate offscreen targets needs renderer design work; no measured throughput, VRAM budget or viewport count is established here.
- Decide exact transparency policy (sorted objects/primitives versus a different algorithm), mask-shadow handling, wire/outline rendering and PBR part support for general articulation before asserting universal parity.
- Decide whether “flat” must remain historical long-lens perspective or become true orthographic; these are observably different picking/projection behaviors.
- Future parity checks should cover DESIGN/WALK/BALANCE/DRIVE/catalogue/live flight, cold/warm/rejected assets, rotated mounts/COM, textured alpha, multiple viewports and depth-aware overlays. These are recommended acceptance cases, not claims that tests were run.

[host]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/studio_only.rs:68
[yard]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/shipyard.rs:233
[dispatch]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:7748
[design]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:2731
[design-submit]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:8186
[stand]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4273
[walk]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4394
[balance]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/test_drive.rs:558
[drive-camera]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/test_drive.rs:668
[drive-submit]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/test_drive.rs:784
[catalogue]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:6078
[app]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/lib.rs:1661
[flight]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/scene.rs:959
[scene-entry]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/scene.rs:20
[observer]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/scene.rs:191
[layout]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/studio.rs:954
[asset-shape]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visual_assets.rs:15
[asset-id]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visual_assets.rs:64
[asset-prepare]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visual_assets.rs:126
[asset-bytes]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visual_assets.rs:203
[asset-faces]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visual_assets.rs:166
[face]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/equipment_visual.rs:12
[projector]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/equipment_visual.rs:149
[pbr-material-import]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:156
[pbr-shader]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/shaders/pbr.wgsl:150
[graphics]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:71
[flight-light]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/scene.rs:35
[canvas-api]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:791
[hud-pass]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/renderer.rs:1226
[canvas-batches]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/renderer.rs:1630
[pbr-pipes]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:683
[pbr-draw]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:1188
[shadow-pipe]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:719
[shadow-draw]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:1214
[shadow-shader]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/shaders/pbr_shadow.wgsl:1
[placement]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/equipment_visual.rs:47
[world-placement]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visuals.rs:18
[mounted-policy]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/mounted.rs:76
[mounted-draw]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/mounted.rs:110
[mounted-compose]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/mounted.rs:154
[uploads]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/renderer.rs:1024
[walk-labels]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4478
[gpu-visible]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:812
[gpu-upload]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:896
[gpu-model]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:1139
[instance]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:706
[gpu-placeholder]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:917
[placed-shape]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/equipment_visual.rs:25
[pending]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/equipment_visual.rs:159
[pending-test]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/equipment_visual.rs:432
[native]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/models.rs:182
[lease]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/pbr.rs:121
[mounted-current]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/mounted.rs:127
[prediction]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visuals.rs:89
[camera]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/camera.rs:4
[walk-camera]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4399
[editor-camera]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4538
[frame]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:99
[globals]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/renderer.rs:991
[passes]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/renderer.rs:1065
[overlay-primitives]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:580
[front]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:423
[scene-pass]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/renderer.rs:1167
[editor-ray]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4568
[picking]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:6537
[pbr-record]: /home/alexm/git/universe-ships-studio-architecture/crates/engine/src/frame.rs:192
[stand-build]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:4177
[craft-build]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/interior.rs:5122
[flight-interior]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/scene.rs:967
[walls]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/models.rs:52
[view-demand]: /home/alexm/git/universe-ships-studio-architecture/crates/game/src/visuals.rs:46
[perf]: /home/alexm/git/universe-ships-studio-perf/docs/diagnostics/studio-walk-investigation.md:3
