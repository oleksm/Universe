# Distributed MC-07 RCS authority review

Added a diagnostic using the actual hull importer, mass/inertia and runtime
allocator, plus an independent bounded positive-thrust LP and preserved socket
fixtures. All twelve signed pure-axis directions have authority at 120 kN and
hypothetical 6 kN, across dry, fueled and full-cargo states. Frame/nozzle checks
and analytical LP controls pass.

Distributed geometry reduces roll authority and worsens fueled/empty allocator
tracking (7.65% worst normalized steady error with lift available versus .567%
current); current full-cargo allocation also has a weakness. Suitable for further
study, not flight/install acceptance. No production physics or tuning changed.
