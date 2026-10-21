# Feature intake before spaceship sketching

Revises Spaceship Building so a whole-ship sketch follows an explicit intake:
small rounds of related questions, inherited answers retained, features classified
required/preferred/undecided, and a consolidated brief confirmed by the owner or
an existing explicit delegation. Unknowns remain visible as permitted exploration
or blockers; silence and proposed defaults do not count as answers.

Adds `ship-rule.feature-intake` and `ship-artifact.sketch-prompt`. G0 requires the
confirmed brief; G1 produces a versioned prompt traced to the brief and reference
atlas; G2 consumes that prompt and reviews sketches against the same feature IDs.
Requirements and identity checks cover feature/prompt omissions and unsupported
claims. Small clarification diagrams remain available during intake, and wording
corrections within confirmed intent do not require repeated approval.

The root now has 92 draft definitions. Updated source archive, coverage map and
hash preserve all 33 detailed sections. No schemas, runtime ship format or ship
instances change, and no modelling is started.

Validation: 20 registry regression tests pass, including source/hash coverage;
full registry checks and headless intake-rule, sketch-prompt and G0/G1/G2 pages
pass. Schema files and runtime code are unchanged.
