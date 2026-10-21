# Planet LOD object boxes

Read the optional planet-forge boxes.json beside a view. Draw oriented, metre-scaled transparent
boxes on the sphere, with shared cube geometry and GPU instancing, terrain depth testing after opaque
geometry, no depth writes or shadows. Cull by horizon/frustum; cap labels at the nearest 40 labelled
objects in view. Family-colour panel toggles and boxes= selection support exploration and repeatable captures.
RGB channels use the producer's byte range; alpha remains 0..1. Invalid sidecars fail clearly.

Validation: geometry/orientation/colour/selection, nearest-label cap, actual 61,179-box/six-family
fixture, and renderer test proving 61,179 instances form one GPU draw with preserved ordering/scales.
Native Vulkan sample captures are in out/review/planetlod-boxes (untracked review output).

Integrated planets' landmark-marker commit 664a45910 alongside boxes; split VIEW controls into
two rows so the new landmarks toggle fits. Combined engine/game library tests: 34 passed, three
fixture-dependent tests ignored; the boxes fixture test was also run explicitly and passed.
