# Reduce the workspace suite to 50 tests

Delete 186 tests and unused test fixtures, keeping 50 fast checks across async
loading, rendering, save integrity, shared controls, registry consistency and
canonical terrain. No production behavior changes. The ignored GPU test is
removed; every retained test runs in the default workspace suite.

Selection and measured timings: `docs/testing/fast-workspace-suite.md`.

Validation: all 50 pass, none ignored; 3.79s summed harness time, no warnings.
Registry build passes.
