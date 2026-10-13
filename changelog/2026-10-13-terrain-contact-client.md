# Shared experimental terrain-contact CPU helpers

Moved the existing geometry implementation into `universe-terrain-contact`, a client library depending on the canonical world reader. World examples share its source without a reverse world dependency or copied implementation. Borrowed mesh accessors retain actual selected geometry without duplicating vertex arrays; typed construction avoids an extra JSON-to-mesh conversion.

Diagnostic subsets bind to the complete selection, reject changed IDs/ancestor overlap, and count their owned array copies against the transient cap. Optional cells cannot coalesce across the mandatory core/band boundary. Runtime publication remains disabled pending incident closure, exterior excision and GPU evidence.
