# MC-07 distributed RCS authority review

2026-10-10. Registry `9c977663` permits the distributed topology for study.
Engine tasks: `20261010T173012-registry-11a9`, `20261010T173230-ships-6996`, and
ships' earlier topology block `20261010T172846-ships-e93d`.

**The distributed layout has positive six-axis authority at both requested
ratings. It is suitable for further isolated integration study. This is not
flight/installation acceptance:** the existing allocator leaves larger roll
tracking errors than the current geometry, and hardware/thermal qualification
remains open. No nozzle, share, device, fuel or production physics has changed.

## Inputs and method

Vendored ships' JSON and meshless GLB under
`crates/core/world/tests/fixtures/mc07-rcs-distributed/`. Their hashes and the
source hull hash are recorded in `mc07-rcs-authority.json`. The GLB's 32 full
frames match the JSON to maximum component error `1.5348196029663086e-6`.
All 20 RCS names are retained; six main and six lift frames match the installed
hull within numerical tolerance. The root transform is applied once and the
actual importer's `made_centre` shift is subtracted once.

`mc07_rcs_review` imports the current `assets/models/mc07.glb` through the real
hull consumer. It uses its fitted dry mass, centre of mass and full inertia
tensor, rather than a uniform rod estimate. It compares current and distributed
RCS sockets at 120 kN/nozzle and a **hypothetical** 6 kN/nozzle, in three states:
dry/empty (156,480 kg), full 4,000 kg fuel/empty (160,480 kg), and full fuel plus
1,200,000 kg cargo (1,360,480 kg). New nozzle/support/feed hardware mass is absent.
Cargo is the current engine's point load at its hold mount, not a measured
spatial cargo distribution; this makes loaded turning results optimistic.

The independent SciPy/HiGHS LP constrains every duty to [0,1], maximizes each
signed acceleration with the other five accelerations exactly zero, and uses
all off-diagonal inertia terms. Each of the twelve signed limits is positive
in all twelve cases. Thus this tests bounded positive controllability, not only
algebraic rank six. Analytical bidirectional and rank-six-but-one-sided fixtures
pass. Equality residuals are checked after row scaling to 1e-7 (a numerical
solver tolerance, not a flight acceptance threshold).

## Pure-axis limits, full fuel and empty hold

Each number is the weaker of the positive/negative pure-axis limits, with no
unwanted linear or angular acceleration. Translation is in m/s²; rotation is
in rad/s². Body X/Y/Z angular axes are pitch/yaw/roll.

| Layout / nozzle thrust | X | Y | Z | Pitch | Yaw | Roll |
|---|---:|---:|---:|---:|---:|---:|
| Current / 120 kN | 1.49551 | 2.83664 | 2.99103 | .335360 | .225000 | .310428 |
| Distributed / 120 kN | 1.29515 | 2.11019 | 5.54259 | .233419 | .192988 | .107180 |
| Current / 6 kN hypothesis | .074776 | .141832 | .149551 | .016768 | .011250 | .015521 |
| Distributed / 6 kN hypothesis | .064758 | .105510 | .277129 | .011671 | .009649 | .005359 |

The distributed geometry trades substantial roll authority and some pitch,
yaw and lateral authority for greater axial translation. At 6 kN, ideal
rest-to-rest 90-degree lower bounds from these limits are 23.2/25.5/34.2 seconds
for pitch/yaw/roll (`2 sqrt(angle / acceleration)`). These omit rate governors,
changing attitude, fuel use, dynamics and actuator response; they are not flown
manoeuvre timings. At full cargo, distributed 6 kN pure translation falls to
.00764/.01245/.03269 m/s²; rotation scarcely changes under the point-load model.

## Runtime allocator concern

The same Rust diagnostic invokes the actual `thrusters::allocate_with` at
zero main throttle, first with RCS only, then with all 32 nozzles (lift available).
Commands are twelve signed axes at 25% of their weaker LP limit, plus two mixed
commands at 1/12 of each limit. These amplitudes are invented **test inputs**,
not operating rules. Mixed commands are inside the convex feasible set.
Outputs are recorded on cold call 1 and call 120 with the real warm start and
RCS forgetting. These are repeated static allocations, not a closed-loop flight.

The report normalizes each acceleration error by that axis's weaker LP limit,
then divides the largest component by the largest similarly normalized target.
At full fuel/empty hold, all-nozzle allocation leaves a worst **7.65%** normalized
error after 120 calls for distributed geometry, versus **0.567%** for current
geometry. It occurs on positive roll and is an actual roll shortfall: at the
6 kN hypothesis, requested .00133975 rad/s², delivered .00123724. Small unwanted
linear/angular components remain. Worst cold mixed-command error is 30.75%.
RCS-only distributed allocation leaves 19.84% mixed-command error; lift assistance
helps. Normalized results are the same at 120 kN and 6 kN in these static tests.

Full-hold results expose an existing baseline weakness too: current geometry
has 36.88% worst all-nozzle error, while distributed has .978%. This supports
reviewing allocator convergence/weighting and cargo inertia assumptions rather
than declaring geometry defective solely from one load state. No arbitrary
pass/fail flight tolerance or new tuning has been imposed. Before flight
acceptance, resolve tracking regressions and run dynamic attitude/translation
cases across load states; separately complete ships' clearance/support/thermal
work. Six-kN product/fuel migration remains a registry proposal.

## Reproduce

Run from the game repository; Python requires NumPy and SciPy:

```sh
~/bin/capped cargo run -q -p universe-world --example mc07_rcs_review -- \
  crates/core/world/tests/fixtures/mc07-rcs-distributed/distributed-sockets.json \
  crates/core/world/tests/fixtures/mc07-rcs-distributed/distributed-sockets.glb > /tmp/rcs-columns.json
python3 tools/review/rcs_authority.py /tmp/rcs-columns.json /tmp/rcs-lp.json
~/bin/capped target/debug/examples/mc07_rcs_review \
  crates/core/world/tests/fixtures/mc07-rcs-distributed/distributed-sockets.json \
  crates/core/world/tests/fixtures/mc07-rcs-distributed/distributed-sockets.glb \
  /tmp/rcs-lp.json > /tmp/rcs-controls.json
python3 tools/review/rcs_authority.py /tmp/rcs-columns.json /tmp/rcs-review.json /tmp/rcs-controls.json
python3 -m unittest discover -s tools/review -p test_rcs_authority.py
```
