# Studio YAML on current assets and loader

Integrated the eight completed YAML authoring commits from ships through
24152fd1 with latest-main and the frozen registry checkpoint 687b8e60.
CODE, Apply, unified saves, full module poses and all design layers now coexist
with shared asynchronous model loading and alpha rendering.

Pending visuals retain the complete authored quaternion when their installed
model arrives. Local structure uses an explicit uncached visual constructor
and opaque material flags. YAML launch validation runs in the preparation
worker, keeping the loading/error shell responsive.

Validation and exact audit: docs/diagnostics/studio-yaml-integration.json.
Existing model motion, fit and ship acceptance gates remain unchanged.
