# Hull mechanisms and exits: bound to the model's exact bytes (discrete states, import side)

Registry `20261023T184542-registry-0607` (fso 4acf049a0, merged: the hull record's `mechanisms`, `exits`,
`model_sha256`).

- `mechanisms::bind`: a record that declares mechanisms or exits binds them to its model only when the bytes' SHA-256
  is the record's (else the import is refused). Each declared node must be in the model exactly once. Mechanism nodes
  and their parents must be unit scale (1e-4). A rigid pose's quaternion must be unit (1e-6, never normalised). A
  state poses every owned node exactly once. Climb and step exits need a foot.
- `Bound::point`, `moves`, `open`: with one state per mechanism, each state's parent-local transforms are set, then
  the node tree is evaluated once. That gives each owned node's rigid move from the export's pose (what hangs from it
  moves with it), a marker's place, and whether an exit is open.
- `ClassSpec::mechanisms`: set by the importer. Nothing uses it in play yet (states on the ship, posed walk floors,
  contact, exits in crew: next).

Test: `mechanisms::tests::mechanisms_pose_by_record_and_bind_only_their_model`.
