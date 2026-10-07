# Shared identities, positions and staff (fso merged)

- `product_identity`, `surface_position` and `staff` defined once in common; equipment, hull,
  structure and gate identities derive from the first; sights' and settlements' positions are one
  type (`SurfacePosition`), buildings' and modules' staff one (`StaffItem`). The generator
  rebases a shape's references when it inlines one from another file. No field name changed.
