# Hull import diagnostic

Added a read-only `hull_import_review` world example for exact candidate exports.
It reports registry versus imported slots, fit instances, nozzle shares and mass,
with explicit dropped/added entries, missing mount nodes and importer refusals.
Reports pin model, registry and consumer source hashes. It adds no hull handle and
writes no saves; differences return exit status 2 with JSON evidence.

Explorer's exact-candidate run awaits Blender's measured hull-only export and
the real registry record. The diagnostic does not register placeholder data or
qualify physical mounts, landing, crew clearances or runtime articulation.

Validated on engine's `ed70fa59b`: 101 workspace tests pass (8 ignored), two
diagnostic tests pass, registry build passes without generated changes. MC-07
comparison correctly reports 13 remaining omitted fit entries while retaining
four inert gear modules and their mass; altered model-path spelling is flagged.
These baseline probes do not accept Explorer. Per-instance output exposes inert
function and zero native power draw alongside the registry's nominal requirements.
