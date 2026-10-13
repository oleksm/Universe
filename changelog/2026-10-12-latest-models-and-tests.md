# Latest model and test checkpoint

Merge approved registry595df19e, completed WALK profiling dd94f934, and the
completed Studio model-basis notes. Preserve the 50-test reduction31f68543.
Ten ore-bay packages advance to rebuilt versions; fan S1/S2 and FT S1/S2 gain
installed models. Registry data also adjusts fan masses and FT efficiency/heat;
FT efficiency0.61 to0.41 increases deuterium consumption at the same jet output.
Existing model validation remains partial where GPU/placement checks are pending.

Model validation is now an operational `validate_models` example. The registry
validation tool calls it directly after removal of the broad visual-loading test;
it does not add a test back to the default suite.

Architecture research and the registry axes/worldmap staging branches remain
outside this ready checkpoint. Validation evidence:
`out/review/latest-main-2026-10-12/`.

Validation: registry build passes;50 workspace tests pass (zero ignored);
all 139 installed model packages load with verified hashes and nonempty bounds.
This is loader validation, not new GPU/placement approval.
