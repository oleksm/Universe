# Spaceship Building definitions integrated

Integrate the separately certified fso delivery `78643bf2` (content `ec25d694`)
into local main after the earlier stable-work merge. It adds 90 draft workflow
definitions, five schemas and registry validator/page support. Existing Rust code,
base game content and the `.ship.yaml` runtime schema remain unchanged. Proposed
authoring/compiler/harness work is not implemented or authorized by this merge.

The sole conflict was the generated page, resolved with the generator's merged
record output. Registry generation, all 31 targeted regression tests and headless
page register/link checks pass without an external registry override. The checks
cover engine compilation/registry decoding, existing packing/save/model behavior,
the celestial guard and 20 registry regressions including six workflow tests.

Exact scope and evidence: `docs/diagnostics/spaceship-building-integration-20261020.json`.
Other held work stays excluded. No remote push or release installation.
