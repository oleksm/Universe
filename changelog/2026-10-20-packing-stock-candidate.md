# Manufactured packing stock candidate

Existing packing now reserves one matching manufactured empty handling unit per
package, counts tare once, and retains the reservation through whole transfers
and saves. Opening returns serviceable reusable units to the source, while spent
bindings and damaged tare become unpacked packaging waste. Empty units trade by
their physical piece mass and draw their supplied package shell without contents
or label geometry. Selling a split checks room for the empty and loose remainder.

Prepared in isolation on `engine-packing-stock`; no merge to main. Registry and
integration details: `docs/requests/packing-stock-candidate.md`.

Validation: 11 distinct targeted tests pass (8 service, 2 save, 1 real-model).
The final combined `cargo test -p universe-services -p universe-sim -p universe
--lib packing` run passes all 10 selected tests; the separate shortage test also
passes. All 5,116 registry record/schema files in the isolated test snapshot match
fso's final delivery, including `b799272d` and policy commit `edf0e64b`.
