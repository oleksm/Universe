# 2026-09-30 — Client/engine split, phase 1: meshes live on the GPU

User: "Definitely meshes need attention. yes so we should start separating client from the engine.
Client just operates my view and does rended. Engine computes the world, it has to use multi
threading I dont think we can skip it. Even client should probably have rended vs world interation
separation"

## The plan (phases)

1. **GPU-resident meshes** (this change).
2. **A command/snapshot boundary between the client (`game` + renderer) and the world engine**
   (`sim`, `world`, `avionics`, `physics`). Everything the client does to the world is a command;
   everything it shows comes from a published snapshot. `Rc` becomes `Arc`.
3. **The world engine on its own thread**, at a fixed tick; the client interpolates.
4. **Parallel crafts tick**: ships compute against a frozen frame in parallel, and their requests
   (pads, corridors, shots) are applied in order after.
5. **A client render thread**, separate from input and view building.

## Phase 1

- **`Mesh`** (engine): a model with a stable id, shared cheaply (`Arc`).
  - Uploaded to the GPU the first time it's drawn: faces with their normals, edges, colours.
  - Kept while in use; dropped after 600 frames unused.
- **Model draws record an instance, not vertices**: rotation × scale, camera-relative position,
  edge and face tints, the star's direction and its brightness here, the reflecting planet's
  direction, coverage and colour.
  - Draws are grouped by mesh, so 300 ships are one instanced draw.
  - `model_shaded`, `model_shaded_faded`, `model_colored_shaded`, `model` and `model_colored`
    keep their meaning.
- **The mesh shader** (`vs_mesh`, `vs_mesh_line`) does the same lighting the CPU did:
  - flat faces;
  - edges dimmed by the light on their ends;
  - ambient;
  - planetshine by view factor;
  - both lights through the eye's adaptation.

  Screenshots are unchanged.

## Measured (headless crowd scenario)

- Planet meshes: 1.45 ms → 0 (on the GPU).
- Draw: 0.8 ms → 0.35 ms. (It was 2.3 ms before the profiling round.)
- The surface grid near the ground is still built on the CPU (0.76 ms when low over a planet). It
  follows the camera, so it's a candidate for the GPU later.
