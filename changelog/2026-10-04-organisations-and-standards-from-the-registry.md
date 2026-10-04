# Makers, standards bodies and standards read from the registry

- **Brands are the registry's makers:** the organisations (`org.*`) that are companies whose
  business is making things. `brands.ron` is no longer loaded. The game's own content names them by
  their registry keys: `org.hadley`, not `brand.hadley`, in modules, hulls and structures.
- **Standards bodies and their standards come from the registry** (`org.*` of kind
  `standards_body`, and `standard.*`), not `bodies.ron` and `standards.ron`. A standard's key is the
  registry's (`standard.sfo.18`). It's cited as before (`SFO 18 V1`), from its body's prefix and its
  number. Its branch is its first topic, as the build had it.
- **The registry's encoding in the binary is MessagePack**, which keeps the shape of values that
  can be one thing or another (a paragraph or several; a number, a range or text).
- Nothing changes in play. The standards register shows the same bodies and standards.
