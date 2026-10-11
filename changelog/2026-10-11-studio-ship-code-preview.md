# Ship Studio YAML export and preview

- Added all-layer legacy-design export to `.ship.yaml`, headless validation and
  direct YAML preview through the shared ship-design document library.
- Added full quaternion equipment orientation to the shared Studio visual path,
  preserving roll in all views and using the same orientation for engine thrust.
- Added registry compatibility checks and no-clobber atomic file export. Invalid
  imports leave the open design unchanged; preview cannot overwrite source saves.
- Kept source-preserving visual edits, stable editor identities, unified deck
  undo/save and the code/clipboard panel pending rather than losing authored data.

Usage and current limitations: `docs/ship-code.md`.
