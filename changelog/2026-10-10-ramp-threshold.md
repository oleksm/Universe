# MC-07 ramp threshold collision review

Added a real Walker/GLB diagnostic and reproducible owner-fixture runner. All 12
isolated heel crossings pass, including the actual deployed chamfer. Full routes
reach their endpoints, but deployed traversal briefly loses grounding outside the
heel. Cargo traversal is unsupported pending a registry carrier/body contract.
Superseded the obsolete native hinge-fit failure with the accepted isolated
heel-pocket interface; replacement remains disabled pending combined-hull/live
five-part integration. No asset install, collision retuning or production change.

Validation: registry build passes; workspace run reports the existing MC-07
empty-hold hover assertion (8.2 m/s² lift). Sent to registry without retuning.
The remaining workspace tests pass with only that named assertion excluded.
