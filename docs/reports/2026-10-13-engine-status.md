# FREEFALL engine progress October 12 to 13 2026

Editable document: https://chatgpt.com/space/page_5d6c79501b508191af71bbbed87de552

## Progress at a glance

Engine work for October 12–13, 2026, through the latest completed handoff. My main delivery was the shared engine foundation for the Studio rebuild, followed by bounded planet contact and streaming work. Ships owned the Studio client integration, final release validation and merge; planets supplied the landscape and independent terrain checks.

The shared Studio and installed-model checkpoint is on main at **0c484a56**. The latest planet contact work is isolated on **engine-pgs-walk**, through **186e4be5**, and is not installed or merged.

```text
Assets loader                         FOUNDATION IN — existing shared service
├─ Model promotion and leases         done (shared world and Studio path)
└─ Terrain replacement lifetime       in progress (16 MiB additional GPU cap)

Studio on the shared game engine      DELIVERED — merged in 0c484a56
├─ Retained scenes and shared physics done (all 5 Studio views)
└─ Repeated device authority solves   done (46 ms → 0.009 ms fixture update)

Latest changes and models on main     DELIVERED — frozen registry cutoff
├─ New installed models               done (12 packages, 48 inspected views)
└─ Combined checkpoint validation     done (67 workspace tests, 151 model reads)

Planet orbit to walking               FOUNDATION IN — isolated experiment
├─ Continuous simulation journey      done (12.82 m walk, 0.445 m jump)
├─ Actual selected terrain CPU checks done (6 cases within 16 MiB)
├─ Moving terrain publication         pending (0 accepted GPU runs for new patch)
└─ Reboard and return journey          blocked (existing hull access)
```

**Headline numbers:** 12 new models checked; 6 actual terrain selections pass CPU contact, topology and seam checks; maximum combined diagnostic memory 15,725,908 bytes under 16 MiB. The current terrain branch passes 72 workspace tests. Test counts describe different snapshots, not cumulative totals.

## Completed and merged

### Shared Studio rendering and simulation

I supplied generic retained scene and viewport support in the engine, including camera and orthographic support, composition with the interface, material handling and resource lifetimes. Ships integrated the editor around those facilities and removed the old CPU model projection path.

Studio now uses the same PBR model handles, renderer and asset cache as the game. Its local flight session calls the existing world and ship simulation. Editor state and trial worlds stay isolated; there is no separate Studio vehicle integrator.

I also addressed shared device support and an expensive repeated authority calculation exposed by the new flight path. In the 12-module validation fixture, update time fell from about **46 ms to 0.009 ms**, and the run reached **120 FPS instead of 22 FPS**. Shared frame statistics now include child views and PBR geometry.

The matched 41-module WALK fixture reduced warm CPU drawing from **2.704 ms to about 0.07 ms**. These are specific fixture measurements. A separate embedded run remained near 33 FPS with roughly 26 ms spent waiting for the display surface; that desktop/presentation confound is documented, not claimed fixed.

### Installed models and release checkpoint

I independently checked the 12 new installed packages through PBR decode, upload and 48 static views. Ships combined this with Studio DESIGN, WALK and DRIVE checks, native MC-07 validation, registry checks, release builds and 151 model-loader reads.

The combined result was merged and pushed as **0c484a56**, with tested game and Studio binaries installed. The registry cutoff is **ddc59ca7**. This accepts the models within their neutral/static visual scope; motion, structural qualification and combined-hull fit retain their separate gates.

### Loader foundation

The shared asynchronous loader, fallback envelopes, promotion to ready models, leases and profiling counters were already established before this reporting period. This day’s work carried that foundation through the retained Studio renderer and extended lifetime accounting for optional terrain replacements. It should not be presented as a new completion of every streaming path.

## Planet walking progress

### Connected the real landscape to the real runtime

I coordinated the v8 full-globe integration and delegated a bounded continuous simulation journey to ships. The successful headless run used the existing shared runtime for coast, descent, landing, exit, walking and jumping without resetting flight state after spawn.

It landed after **2,330.14 simulated seconds**, walked **12.82 m** and jumped **0.445 m**. Reboarding failed because the tested hull lacked a valid interior access destination. A failed first landing attempt remains archived. This demonstrates a partial simulation journey, not a complete rendered player experience or robust guidance from arbitrary starts.

The lab preserves the registered host radius and physics. The source landscape has a different radius; directions and heights are preserved and the horizontal scale difference is declared.

### Corrected terrain representation near the feet

The canonical terrain sampler and the displayed mesh could disagree near creases and patch boundaries. I traced failures to optional subdivision slivers, float32 collapse and inconsistent ownership of shared coarse/fine edges.

I implemented bounded optional-cell coalescing and finite-edge ownership correction. Missing owners, stale selections and unresolved geometry fail explicitly. Source geometry, physics and acceptance tolerances remain unchanged.

The original full 128 m contact gate failed and remains failed. The current experiment defines a **64 m contact core inside the same 128 m footprint**; the transition band is outside the walking-readiness claim. Wet visual geometry uses the water surface level, not the submerged bed.

### Validated actual selected meshes

Initial settled-grid controls passed at seven fixed sites and seven shifted anchors. I then moved to six snapshots exported from the real terrain selector: three difficult locations, each during loading and after settling.

All six pass CPU contact, topology and seam checks. Sampled core height error is at most **5.864 micrometres** and the complete outer-segment support bound at most **4.120 micrometres**, within the existing 10-micrometre seam tolerance. These are bounded geometric checks, not a claim of micron-accurate terrain or completed GPU validation.

The largest combined transient allocation, including diagnostic snapshot array copies, is **15,725,908 bytes** against a **16,777,216-byte** limit. Runtime inputs can instead borrow the already resident mesh data.

### Shared worker and memory ownership

I extracted the CPU implementation into **universe-terrain-contact** at **186e4be5**, so the runtime adapter and diagnostics use one implementation. The world layer does not depend on client intentions. Ships has the API handoff.

The earlier **792f2bac** engine change adds whole-group mesh reservations and cache-residency receipts. Reservations survive CPU handles, frame submission, cached GPU buffers and retirement. It provides accounting and lifetime support; it does not by itself certify a replacement as safe to display.

## Remaining work

The planet patch is **not yet runtime-ready**. Three engineering steps remain before the next accepted GPU run:

1. **Close all moving-edge dependencies.** Holding a coarse endpoint fixed can affect fine vertices farther along an edge. The worker must include that dependency set or refuse within its caps.
2. **Replace terrain without holes or overlap.** Retain the exterior pieces of crossed triangles, share boundary splits, and handle skirts and edge aliases. Simply drawing a patch over existing terrain is insufficient.
3. **Publish the complete group atomically.** Recheck source and selected-mesh bindings after upload, retain ordinary terrain while pending or refused, and account for active, uploading and retired allocations.

After those CPU prerequisites pass, the next gates are fixed and moving-camera GPU checks, delayed/refused loading, eviction and return, and telemetry bound to what was actually submitted. The declared limits remain **0.10 m contact error**, **33.3 ms p95 frame time** and **16 MiB additional GPU lifetime budget**.

The full reboard, takeoff and return journey is separately held on verified hull access. No fabricated floor or forced seat teleport was introduced to manufacture a pass. No new terrain source, default installation or planet merge has been made.

## Evidence and handoffs

The report separates my engine changes from collaborator integration and validation. Repository paths below identify the reproducible evidence.

| Delivery | Commit or evidence |
| --- | --- |
| Merged Studio, engine and model checkpoint | Main 0c484a56; universe/docs/diagnostics/ready-main-2026-10-12.json |
| Shared Studio architecture and fixture timings | universe/docs/studio-shared-engine.md; docs/diagnostics/studio-rebuild.json |
| Headless planet journey and access failures | ships-pgs-journey commits 04b47fb6 and 1dc4906c; docs/diagnostics/v8-runtime-journey.md |
| Mesh group reservations and receipts | engine-pgs-walk 792f2bac; docs/diagnostics/mesh-group-budget.md |
| Boundary correction | engine-pgs-walk a4ae5ea9; docs/diagnostics/v8-boundary-correction.md |
| Shared terrain worker API | engine-pgs-walk 186e4be5; crates/terrain-contact |
| Six actual-selection CPU audits | universe-engine-pgs-walk/out/review/v8-boundary-correction/actual-resident-v1 |
| Actual selector snapshot producer | ships-contact-adapter 104470d8 |

The next concrete milestone is a bounded, complete terrain replacement that stays valid while the camera moves. The current evidence clears the sampled CPU geometry stage only.

