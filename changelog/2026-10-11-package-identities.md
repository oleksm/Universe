# Package identities and transfer accounting

Generate runtime labels from the registry's label schema. The services packing primitive fills a single lot, seals full or explicitly closed partial units, and refuses suffix reuse beyond 9999. Whole sealed units retain their labels on transfer; splitting or consuming one opens it and drops its packaging while the stock retains its lot. Pool and ledger package metadata carry historical tare and outside volume; market capacity checks include both.

This is the state/API foundation on engine-package-labels. Automatic production packing, save/restore and drawn label textures are subsequent integration work; no new packages are created by the economy yet. No main merge.

Validation: services unit suite, including accrual, partial sealing, immutable transfer, split/repack, loose-first selection and suffix exhaustion.
