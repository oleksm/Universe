# Latest completed code and installed-model checkpoint

Integrates registry c288b4ab, 42 changed installed model records (including CH-S1/S3/S4), completed Studio YAML authoring/editor code, and PGS ownership precision fix860e6802. Shared asynchronous assets, alpha passes, frame leases and pending-model rotations are retained. All134 prior installed models passed package verification and actual PBR decode; the additional CH-S3 passed a Vulkan async-loader fixture.

Model installation is not motion or engineering acceptance. Ten ore-bay meshes still use20mm unribbed geometry while the revised records specify8mm ribbed construction; rebuilding them remains explicit work. Globe initialization still has the previously measured4.48s first-use stall. Final validation is recorded in docs/diagnostics/latest-checkpoint-2026-10-12.json and studio-yaml-integration.json.

Final frozen registry c288b4ab:232 workspace tests, registry build, both release binaries,8 CLI cases and4 Studio Vulkan captures pass. Preview axis mismatch for new -Z-thrust assets versus +Y design convention is pre-existing and remains explicit follow-up; the YAML merge preserves authored rotations.
