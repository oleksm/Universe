# Explorer physical material and stock schedule

Choose physical materials and existing raw feedstock for all 952 current objects in 17 owned assemblies at source `8fc3a7e3`. The schedule assigns 11 functional material roles, preserves registry densities and actual reconciled solid volume, and gives concrete nominal sheet/plate targets without silently thinning the drawing. Paint does not determine material.

`docs/ships/explorer-material-stock-schedule.yaml` records every exact object assignment, source hashes, stock references and explicit prototype design assumptions. Add 96.1 kg of separately stated dry hardware/actuation reserves; replace corresponding reserves when hardware is actually represented. The separate 15% metal procurement offcut estimate never becomes installed mass. Roof tank/cradle and catalogue equipment remain outside this source schedule.

Validation: 952 unique assignments, 17 assemblies, 21 pinned input files verified, six semantic assignment checks (including panel/pane, sealed returns and quarter-turn fixings). Audit: `/home/alexm/.agents/reviews/registry/20261023-material-schedule-check.json`. These checks establish assignment integrity, not geometric volume, whole mass or load qualification. Blender owns current solid/overlap reconciliation and the resulting mass calculation. No active registry record or schema changed.
