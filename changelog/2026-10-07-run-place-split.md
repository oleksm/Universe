# The economy's place step in four parts (audit phase 5)

- `Economy::run_place` (240 lines) is the step's outline: its works gathered as one record each
  (`AtWorks`: which works, its owner, who marks what it makes, how it ran, the power it drew), then
  `produce` (setups run as far as power, inputs, store and deposit allow), `mark` (serials and
  lots, SFO 21), `burn_and_bill` (stations burn for what was drawn and bill who drew it) and
  `trade` (spare sold to the warehouse, inputs bought to cover). Five parallel vectors indexed
  alike are gone. Same numbers: the economy tests and replays pass.
- Needless borrows clippy found in the scenario table (`&sys` where `sys` is a reference now).
