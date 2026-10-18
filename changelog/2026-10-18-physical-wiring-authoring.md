# Physical wire authoring foundation

On the `ships-physical-wiring` branch, ship documents can preserve an optional
`wiring` collection. Each entry names its two equipment instances and port IDs,
the connector product at each end, the cable run product, conductor-to-contact
assignments (`pins`), and interior route waypoints in metres. Endpoint positions
belong to the equipment ports and are not duplicated in the saved route.

Local validation rejects missing equipment instances, reused sockets, duplicate
conductor/contact assignments and invalid routing coordinates. Registry checks
for channel, polarity, direction, mating halves and ratings are still required.
Studio preserves these entries through source edits, save/load and undo/redo.

This is preparatory work, not the wire tool release. Explicitly wired designs
cannot be compiled for flight until port resolution and runtime topology land;
they must never silently use the old inferred harness. Existing documents with
no wiring field retain their current behavior. Nothing modifies saved commuters.
