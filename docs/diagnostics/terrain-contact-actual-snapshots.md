# Actual resident terrain snapshot validation

CPU-only selector foundation on `ships-contact-adapter`. Main, physics and GPU rendering are unchanged.

Four scoped terrain tests passed (0 failures), including the explicitly invoked external PGS scheduler/export fixture. Compilation took 26.34 seconds and the tests took 14.86 seconds in the observed run, under the shared scenery launcher with two CPU threads, 8 GiB and a 600-second timeout. This is not a workspace or performance acceptance gate.

The fixture loads frozen PGS SHA256 `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`; the host radius is 6,281,370 m. Camera is fixed at each feature query's first direction, 1.7 m above its canonical visible surface. A minimal CPU GlobeMap provides identity only; no material upload or GPU submission is claimed. Both drawing and this fixture use the original actual resident worker/fallback selector.

| Snapshot | Full selected IDs | Nearby geometry patches | JSON bytes |
|---|---:|---:|---:|
| canonical_crest_1-parent.json | 640 | 122 | 4492806 |
| canonical_crest_2-parent.json | 385 | 112 | 4131392 |
| parent_face_join-parent.json | 424 | 57 | 2161038 |
| canonical_crest_1-settled.json | 942 | 174 | 6374133 |
| canonical_crest_2-settled.json | 933 | 177 | 6455128 |
| parent_face_join-settled.json | 953 | 187 | 6548094 |

All six outputs have zero partial selected entries. Parent exports retain actual ongoing pending work; settled exports have zero pending requests and three consecutive complete selector iterations. Geometry arrays include actual raw mesh vertices and morph channels, and only the first 512 grid triangles as faces. Raw meshes retain skirt vertices; the reader must use the indexed grid faces, not infer extra topology. Geometry is limited to a conservative 128 m tangent footprint plus one actual same-face incident ring; full selected metadata remains available for validating ownership. This subset does not prove cross-face or finite-owner closure: the consumer must refuse absent dependencies. Streaming serialization borrows existing arrays. Diagnostic array copies made by the external reader must be added to its measured working-memory budget.

Every scheduler iteration is preserved in `*-states.jsonl`, including root-pending and loading states. These are paused-camera **CPU selection** traces with `ready:false`, not submitted-geometry readiness traces. No GPU was used and reported extra GPU bytes are zero. Exact-eye invalidation supports stationary snapshots only; moving-job admission, retained Surface Arc identity, incident locking/excision, GPU group upload, atomic suppression and active/uploading/retired GPU validation remain unimplemented or unaccepted. No corrected group is published.

Artifacts: [exports](../../out/review/contact-adapter/actual-snapshots/exports.json), [hashes](../../out/review/contact-adapter/actual-snapshots/hashes.json), [test log](../../out/review/contact-adapter/actual-snapshots.log). The `out/` directory is local review evidence and is not checked into Git.

## Source/selection generation follow-up

The follow-up keeps the physical Surface Arc alive and binds its identity in both source invalidation and the snapshot stamp. Eye movement now creates an immutable per-job view without advancing source/selection generation or duplicating the retained patch slice. Old jobs retain their recorded eye and remain current only while the source and actual selected geometry are unchanged. A regression checks shared leases and unchanged generation across movement, then cancellation on mesh remake/map replacement.

The four scoped tests passed again: compile 10.46 s; tests 14.50 s. The external fixture also verifies retained source Arc equality and writes fresh six-output snapshots to [source-generation-snapshots](../../out/review/contact-adapter/source-generation-snapshots/exports.json), with [test log](../../out/review/contact-adapter/source-generation-tests.log). The original six snapshots above remain valid evidence for the earlier stationary run. This follow-up fixes worker cancellation; it does not establish moving support, full-skirt replacement, closure, excision or GPU readiness.
