# Cutting modules and not-made kinds from the records (fso merged)

- `cuts: true` on the module records that cut stock (welding bay, machining centre, cutting table,
  electronics works): the derived parts read the set from the records (the Rust list is gone).
- `x-in-game: "not made"` on each equipment kind's alternative, where the generator reads it:
  33 kinds flagged; the handler trait's `not_made` implemented once in `modules.rs` (the audit's
  bug 3). Also: heavy members for the ships (4340 and Ti-6-4 tubes, 4340 plate, box sections, a
  section line at the Trethi mill).
