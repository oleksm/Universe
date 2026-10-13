# Studio shared engine architecture research — 2026-10-12

Added [the architecture report](<../docs/reports/Studio shared engine architecture.md>), synthesizing all four specialist notes and earlier controls, installed-model and WALK diagnostics evidence against exact baseline `31f6854326e1a8e78f71cab47c9ccaee7b22379e`.

Documents already shared entrypoints/primitives versus duplicated compilation, rendering and runtime orchestration; a three-layer shared engine/domain/authoring boundary; source-anchored gaps; proposed APIs, revision/identity dataflow, staged deletion gates and completion criteria across hosts/modes. Preserves experimental simulation features and fit/readiness/articulation gates. Proposes consolidating parity coverage within the owner's 50 fast tests, with separate bounded visual/performance tools.

Research only: no runtime implementation, builds, tests, or new benchmarks. Later reported main `b0d0da15` changes were not reaudited. Architecture and acceptance criteria are proposals, not shipped behavior.
