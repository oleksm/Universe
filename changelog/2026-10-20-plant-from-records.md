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

## Kit pieces

Each run whose template is `sweep-and-pieces` is laid of its kit model's pieces once that model is
loaded: the template's own model, else its `model_of` member's, split into parts by node name
(`visual_assets::request_parts`). A 90° corner is the elbow piece, scaled by the run's elbow radius over
the radius the model was built at; a corner of any other angle stays a swept bend. Each support is its
head with a leg from the head's `leg_top` to the ground. Blind ends are the cap, and each branch is the
tee (`world::plant` now gives every run its tees: one at each tapped port, one where a branch ends
`tee:<run>`; their totals match the registry's counts). A conveyor's head and tail stand at its ends with
their legs, and the straight stops their `transition` short. Idlers and return rolls are set along the
straights at their pitch. Pieces scale across and up by the run's `outside` over the model's member.

The straights stay swept, except a belt's: its template profile is the troughed belt on its stringers,
which a swept box would hide the idlers in, so the model's `straight` piece is stretched along each leg.
A run whose model lacks a piece keeps that part swept, and a run whose model hasn't loaded is swept
whole. On the airless shaft furnace 118 of 175 runs are laid of pieces; the rest are cables (a sweep by
their template) and runs whose template has no installed model yet. `Frame::model_pbr_scaled` now takes
the part to draw.
