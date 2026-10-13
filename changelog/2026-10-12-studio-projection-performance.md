# Stream Studio equipment projection

Dense installed equipment made Studio CPU-bound: the shared projector allocated
vertex and clipping vectors per visible triangle, then assembled a full projected
mesh only to traverse it again for canvas submission.

Use fixed three/four-vertex scratch storage and stream projected triangles into
the consumer. Skip shading for triangles wholly behind the near plane; bypass
clipping for fully visible triangles. DESIGN, catalogue, BALANCE and TEST DRIVE
submit directly. WALK collects into its existing ordered scene list without a
second intermediate mesh. Installed geometry, shading, material modes, clipping,
asset leases and physical geometry retain their existing behavior.

A reference-projector regression covers all near-plane classifications, sidedness,
materials and rotated placement/parent frames. Measurements and verification are
recorded in docs/diagnostics/studio-projection-performance.md.
