# Package identity through save and restore

UniverseSave version 3 adds a `packing` snapshot beside the legacy player hold.
It keeps all live pilots' quantities, product marks, lots and physical handling
units; each works' inventory, open lots, partial units, in-progress production,
consumed part serials and remaining deposit; and both product/lot sequence counters
and the per-lot package suffix counters. The economy's production clock is saved too.

Stock is stored by registry key with exact fractional quantities, independent of
the rounded hold display. Recorded works match by facility/rig key and site;
unrecorded player-built works additionally use their site-local slot. Ground sites
use settlement keys; rig sites use the charted system/body indices. Pool capacity
continues to come from the live facility rather than being imported from a save.

Quick-load retains the current NPC population, as agreed with engine. It clears
live holds and restores only matching pilot IDs, checking NPC name and hull key.
Vessel IDs currently append rather than recycle; these identity checks also reject
a replaced slot. Missing pilots are not recreated. Every live ship gets its gross
cargo mass and volume recalculated. Works absent from the snapshot retain their
facilities but start with empty inventory/packing state, so newer labelled packages
cannot survive against restored allocator counters. Missing stock/works records
are discarded, while saved allocation counters remain reserved.

The snapshot replaces state on repeated load; it never appends duplicates.
Restoring works refreshes the market pool copies and invalidates both public views.
Sealed labels and their historical net mass, tare, dimensions and timestamps are
copied unchanged. Partial units remain partial. No labels are inferred from
unlabelled legacy cargo. Older saves without `packing` retain their previous
quantity-only player-hold behavior.

Trade recomputes cargo through Ledger's physical-package accounting. Shock breakage
unloads provenance before zeroing the ledger quantity, removing the broken units'
labels and tare. This does not change the registry's shock thresholds or packing
rules. The core services own sealing, splitting, rollover and production packing.

WorldSave remains input replay. Its existing replay regression additionally compares
the entire packing snapshot, covering quantities, provenance, works state and
allocator state rather than relying only on the older world fingerprint.

This is inventory persistence, not a full quick-save of land ownership, NPC flight
or market population state. The existing replay save remains the whole-world path.

Validation on `ships-package-save` (implementation `034bd693`): registry build
passes; all 144 workspace tests pass, including nine save tests and the replay
snapshot comparison. Commands used an absolute `UNIVERSE_REGISTRY_ROOT`, the
shared Cargo target, and the bounded two-CPU/eight-GiB runner.

Dependencies: engine `a962a100`, `03cedb17`, `e71a7556`, `f68a3d83`; registry
validator fix `5873279c`. This validation covers that foundation and the persistence
integration. Engine's subsequent production packing/roundoff repair is a separate
handoff and has not been included in this branch's results.
