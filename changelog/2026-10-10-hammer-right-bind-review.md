# Hammer-right static bind review

Independent review of the 20-node right mining hammer export passes against all
112 supplied pose references. Maximum bind matrix component error is 3.93e-7;
maximum parent composition error is 1.74e-6, within the existing 2e-6 gate.
All reference node sets and parent relationships match the export, matrices are
finite affine proper rigid frames, and the GLB has no clips, skins or morphs.

`docs/diagnostics/mc07-hammer-right-bind-check.json` pins both input hashes and
records actual rest controls. The scalar control is a fixture index, not a
mechanical input. This checks static binds and reference consistency, not source
vertex reflection or execution of the mechanism. Ships' nominal zero-clearance
chisel/well contact at deployment 0.375 remains a moving-runtime hold.
No asset installation or runtime enablement is part of this review.

Validation: independent fixture audit passes; registry build exits clean; all 131
workspace tests pass. Rechecked the corrected driver-target metadata fixture
SHA256 fd5465ac1b0acec662b8ebb9dfd28ac7ee698f37db7571c2b75d2d1ff175fd09.
