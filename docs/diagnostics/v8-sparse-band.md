# Sparse canonical contact band: CPU results and refusals

The bounded sparse construction builds four of seven fixtures with valid indexed disk topology. It still **refuses publication**: the parent join contains an unrepresentable f32 triangle, and two crests disagree at a shared retained-grid boundary. No GPU integration, source change, physics change, installation or main merge follows.

## Construction and protocol clarification

The frozen plan is `out/review/v8-sparse-band/plan.json`. This implements its sparse algorithm rather than the superseded dense overlay wording inherited in the plan's `transition` field. Canonical 8 m cells span the unchanged 128 m square. Only the outer interface receives detailed retained-grid splits; the 64 m square core remains canonical. The band is never walk-ready. Wet geometry uses water level, separately from bed contact.

Shared retained constraints use the cube face and global level-19 lattice coordinates. Axis-aligned segments on the same grid line share the identity of that line's intersection with the clipping boundary; diagonals use sorted endpoint identities. This extends the plan's physical-endpoint identity to coarse/fine grid representations. Evaluation orders endpoints identically. Reused identities must agree physically within the unchanged 10 micrometre seam limit, otherwise construction refuses. Proximity alone never creates an identity. The existing canonical clipper remains the source of canonical/cell intersections; its parent-join collapse is not repaired by dropping a triangle.

Output band vertices blend canonical visible radius with the reconstructed retained triangle plane using the prior tensor smoothstep taper. Retained selection is the ideal settled CPU reconstruction of existing level-19 grid/morph arithmetic, **not actual asynchronous GPU residency**. Source SHA256 is `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`. Host radius stays 6,281,370 m; source radius is 6,371,000 m. Directions and heights are retained, horizontal lengths use the labelled host scale.

## Frozen results

| Feature | Triangles | Result |
|---|---:|---|
| Shore | 1,794 | Built for CPU audit |
| Parent join | — | Triangle 483 collapses in f32; distinct UV points near (0,0), owner 40805 |
| Sampling-window join | 2,436 | Built for CPU audit |
| Relief/native join | 2,846 | Built for CPU audit |
| Crest 1 | — | Shared retained constraint differs by 0.720 mm |
| Crest 2 | — | Shared retained constraint differs by 0.141 mm |
| Crest 3 | 3,049 | Built for CPU audit |

All 20 original collapse witnesses reproduce (9 shore, 11 parent). The saved regression records source face/body, clipping boundary and nearby retained-edge identities. Nearby candidate associations are diagnostic, not authorization to weld. Reversing the retained edge orientation preserves intersection identity and position exactly in the small regression.

The four outputs pass independent edge incidence/orientation, connected-component, boundary-cycle, Euler-characteristic, positive chart-area and physical f32 nondegeneracy checks. Each covers 16,384 square metres as one indexed disk, with no unexpected interior edge. These are finite/combinatorial checks, not a general proof for arbitrary inputs.

The independent height audit checks 70,875 triangle samples. Maximum core visible-height error is 3.731 micrometres, below the unchanged 0.1 m gate. Wet core samples do not imply dry walkability. The crest-3 band still differs from canonical relief by 0.225632 m; the prior full-footprint contact failure remains. Normal errors also remain, including 34.48 degrees at core samples and 78.08 degrees in the crest band; no normal-quality acceptance is claimed.

Outer physical f32 edges are sampled at endpoints, quarters and midpoint against retained triangle planes: 23,655 samples, maximum radial disagreement 3.716 micrometres. Strict support tests miss 10/4/4/8 samples respectively; all find support within the diagnostic 10 micrometre band. These misses remain disclosed and do not certify a watertight join to the real renderer. The two refused crests exceed the seam threshold before output.

## Limits and next gate

Built outputs use at most 2,103 vertices, 3,049 triangles and 281,610 serialized bytes. Construction timings were 10–18 ms per fixture, excluding resident source loading. Estimated expanded GPU geometry is at most 512,272 bytes per pair; actual GPU allocation is zero. Whole-process peak RSS was 1,683,216 KiB including the resident reader. Per-worker transient allocation has not been independently measured against the 16 MiB working budget, so no memory-residency acceptance follows.

The unchanged limits remain 4,096 source visits, 32,768 vertices/triangles, 2 MiB serialized output, 16 MiB worker scratch excluding resident source, and 16 MiB additional GPU lifetime allocation. No caps were enlarged. GPU lifetime reservation, movement inside a verified core, pending/refused readiness, cancellation, eviction and actual retained-state joins remain untested.

The next correction must resolve canonical/cell intersection identity and retained-grid representation disagreements without proximity welding or tolerance changes. These small witnesses are sufficient to block GPU publication; rerendering is unnecessary.

Machine-readable results, source/artifact hashes, refusal witnesses and validation outcomes are recorded in `v8-sparse-band.json`. Large local evidence remains under `out/review/v8-sparse-band/`.

Validation: 68 release workspace tests pass; registry build exits 0; Python syntax and diff checks pass. These checks do not reverse the geometric refusals.
