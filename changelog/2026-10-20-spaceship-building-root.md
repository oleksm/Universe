# Spaceship Building workflow definitions

Adds the owner-authorized `SpaceshipBuilding` root, split from the detailed
Blender workflow proposal. It holds 90 draft definitions: 9 design stages,
15 authoring artifacts, 29 checks, 24 rules and 13 decision types. All 33 detailed
source sections are mapped in `docs/spaceship-building-coverage.yaml`; a normalized
source archive and its hash retain the detailed proposal and research provenance.

Five closed schemas distinguish reusable workflow definitions from ship projects,
instances and evidence results. The validator enforces path/key agreement,
reference kinds and acyclic stage/artifact prerequisites, with duplicate edges
rejected. Rework and related-check links are not prerequisites. The registry page
shows a separate root, cross-referenced definitions, readable procedures, code
examples, tables and source links.

The existing `.ship.yaml` runtime schema is unchanged. The proposed sidecar,
regional compiler, complete runtime harness and pilot remain proposed; defining
checks does not execute them or qualify a design. Project values, decisions,
evidence and release state remain in their existing authoring/delivery systems.
No ship instance or modelling authorization is created, and held work stays held.

Validation: final schema/reference/dependency and registry checks pass; the
engine registry reader compiles with the five new schema types. Twenty registry
regression tests pass, including six workflow tests covering cycles, duplicates,
malformed prerequisites, rejected instance fields, path keys and source coverage.
Headless page checks cover the root, register, architecture gate, runtime boundary
and coordinate negative-test definition; a rendered page was inspected.
