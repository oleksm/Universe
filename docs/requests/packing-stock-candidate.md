# Manufactured packing stock candidate

Originally prepared on `engine-packing-stock`, based on main, with no registry
files copied into its tracked patch. The completed integration is recorded below.

The existing output-stamping path reserves one matching manufactured empty stock
when it creates a package. Subsequent fills use the same reservation. The
package's mass/volume capacity still limits the contents, and a partial package
uses one empty. Missing stock leaves output loose; it does not create shells or
alter recipe quantities. Packing converts existing stock mass into tare rather
than adding tare again.
No automatic empty-stock procurement or backlog repacking is introduced. Stores
must receive the matching units; shortages stay loose.

Whole sealed packages carry their reservation and label to the destination.
Opening ends the label and releases the reservation at the source: an intact
unit returns only when its stamped `returns_empty` policy and `serviceable` flag
allow it. Spent bindings and damaged units become the package's `waste_stock`,
unpacked and kilogram for kilogram. Damage withdrawal explicitly prevents empty
returns. The ledger records the release with the cargo operation's tick/cause.
Sales reject a split if the returned empty and loose remainder would exceed the
source hold's volume.

Reservations store stock keys, manufacturing lots, serviceability and disposition
policy in the existing package save data. Historical saves with no reservation
remain readable; their shells become waste on opening and cannot mint empty
manufactured stock. Policies for newly reserved units are stamped so a catalogue
edit does not retroactively change them.

Empty handling units trade by the piece, using their physical mass and outside
volume. Shared model requests resolve their `supplies.item` to the package visual.
An empty model's part zero omits `contents` and label nodes; filled and empty
decode options have separate cache identities. This adds support to the existing
shared visual loader; it does not create a new inventory scene or placement flow.

## Registry prerequisite and validation

The candidate uses the manufactured stock records from fso `b799272d` and the
follow-on package policy commit `edf0e64b`:
`returns_empty`, `waste_stock`, and `stock.packaging-waste-bulk`. Missing opening
policy leaves output loose rather than assuming a reusable package.
All 5,116 registry record/schema files in the test snapshot match the final fso
delivery byte for byte, including all package policies and waste records.

A branch-local, ignored snapshot is available at
`out/review/packing-stock/registry`. Build commands use
`UNIVERSE_REGISTRY_ROOT` pointing at that directory; `CARGO_TARGET_DIR` points at
`/home/alexm/git/universe/target`. Heavy commands use `~/bin/capped`, with a
12 GiB memory cap, two Cargo jobs and a 540 second timeout.

Validation results are recorded in the matching changelog entry.
The checks cover partial fills, shortage, serial exhaustion, transfers, splitting,
source volume, spent/damaged tare, legacy packages, reservation save/load and a
real skip GLB's empty shell. They run without opening a test window.

## Integration completed

The owner authorized integration of all stable work on 2026-10-20. Registry
`94cb9b0c` and candidate `0df4427c` were combined in merges `3de437a8` and
`ab1ab81e`. The combined code passed its packing, package, save, real-model,
plant and celestial checks against branch-local standards without a registry
override. Registry generation and 14 Python regression tests also passed.

The full branch audit is `docs/diagnostics/stable-integration-20261020.json`.
The original candidate branch remains available as its preparation record.
