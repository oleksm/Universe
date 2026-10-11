# Shared typed ship document foundation

Add universe-ship-design, a pure YAML1.2-profile authoring library matching the
ship-as-code draft. Typed all-layer documents, generated JSON Schema, canonical
emission, stable-ID reference checks, source locations and explicit parser limits.
No Studio UI, save migration, dependency installation or live ship changes.
Canonical emit does not preserve comments; parse_source retains original text.

Validation: all-layer YAML/JSON round trip and eight focused parser/schema/semantic
checks pass at foundation handoff. Further edge-case evidence lives with crate tests.

Follow-up: quote emitted strings consistently with the YAML1.2 profile, produce
module field-level diagnostics, and cover deck references plus a1000-module
round trip. Thirteen crate tests pass; generated schema snapshot matches types.
