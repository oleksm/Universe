# Explorer floor return stock choice

Choose 2 mm 6061 sheet (`stock.al6061-sh-2`) for the two surface-only perimeter returns 2/6 and the malformed tapered return 5. These are nonstructural edge closeouts. Return 5 has a real visible taper and must be reconstructed from its paired edge paths, not retired wholesale because its old signed volume is invalid. Duplicate/internal sliver faces get an explicit retirement map.

`docs/ships/explorer-floor-return-stock.yaml` pins the raw measured triangles, gives actual edge vertices and normal-thickness construction, and keeps material/density unchanged. The two long reference surfaces each measure about 0.333257 m2, with an uncut 2 mm planning mass about 1.800 kg each; final fold/trim quantities come from closed solids. The stock realization overrides the earlier nominal gauge suggestion for these three objects only. No pressure/load rating or whole mass is inferred.
