# Plants laid from their records

`world::plant::lay(layout)` lays a works from its registry records alone (SFO 24 to 26), with no layout
model. Its units stand at their bays, turned as the bay says. Each run follows its link's path (from,
the corners, to), with an elbow at each corner of more than a degree and its supports placed by the
kit's rule (`plant_kit.py`, `lay`): one at the top of each leg rising more than half its length, else one
every spacing along the leg. A joint (the port's connector, else the run's) stands at every port a run
ends on or taps, at the port's mating plane and facing; load ports get none, and blind ends (`cap`)
are capped. Each run and joint carries its family template. `template_model` says which model draws a
template (its own, else its `model_of` member's installed one) and `joint_scale` scales a connector to
the member that model was built at.

Faults are listed, not drawn round: a missing unit, run or port, and a run end farther than 5 cm from
the port it names (hoods, belt transfers and chutes stand off by design). All 16 layouts laid of runs
lay without a fault, and on the trial airless shaft furnace every run's elbows and supports are as many
as the registry's generator counted.

The asset lookup and `validate_models` now take units, runs, connectors and templates: all 521
installed models read. Nothing draws a plant yet; the renderer is the next step.

## The plant viewer

`freefall --app plant [layout key]` draws a layout snapped together from its records. It lists the 15
layouts laid of runs. Units are their installed models standing at their bays, or a labelled box at
their size while none is loaded. Runs are swept along their paths at their `outside`: round, or a box
section for belts, drag conveyors, buses and conductor rails. They bend round each corner at the run's
elbow radius (a cable's riser radius), cables lie as many circuits as the link has, and colour shows what
a run carries. Stands go from under the run to the ground, and blind ends get a plate. Each joint is its
family template's model: the template's own, else its `model_of` member's in the older kit frame (the
run's axis along +X), stretched per axis to the connector's figures. A joint with neither is a disc at
its size. On the airless shaft furnace all 50 units draw as models, and all 165 joints by template.

The engine gains `Frame::model_pbr_scaled`, which stretches a model along its own axes. Its normals turn
with the stretch, which is close enough at a few times.

Not yet drawn from the templates' pieces: elbows, supports, a belt's idlers and terminals, and tees.
They are swept or drawn bare for now.
