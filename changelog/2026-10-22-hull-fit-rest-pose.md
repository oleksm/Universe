# Explicit rest pose for a fitted motion product

A hull fit entry may now declare `pose: rest` for one equipment instance. It opts that instance into its installed model default-scene rest transforms with motion held; it does not replace the equipment identity or modify its immutable motion package. Absence retains the existing consumer refusal.

The validator requires the referenced equipment visual manifest to match its pinned hash and product key, declare motion, and provide the matching motion file size/hash. Static, absent, stale or mismatched packages fail. The hull page labels the fixed rest scope. Runtime import and rendering remain Engine-owned integration work.

Validation includes valid rest, absent opt-in, static package, missing/mismatched files, wrong product and invalid enum cases. No live hull opts in yet; no Explorer fit or physical acceptance is inferred.
