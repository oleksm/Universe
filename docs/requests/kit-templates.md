# Request to the engine: draw kit runs and connectors from family templates (2026-10-17)

From the registry. The owner, 2026-10-17: never one model per size. Until today every pipe, cable, flange and
termination size was its own installed model (20 cables, 28 pipes, 29 flanges, 20 terminations, all one shape scaled).
The registry now holds one **template** per kit family (`standards/SFO/metadata/templates/`, schema
`SFO/schema/template.schema.yaml`), 21 in all, and every run and connector names its own (`template:`).

## What the engine is asked to do

1. **Sweep** (`draws: sweep`, today `template.cable`): draw the run's straight as its profile swept along the layout
   link's path (`from`, `via`, `to`), the profile sized by the record (`outside`). No model.
2. **Sweep and pieces** (`draws: sweep-and-pieces`: pipe, lagged pipe, lined and jacketed ducts, belts, drag conveyor,
   busbar): the straight swept as above; the pieces (elbow at each corner, support at each `support.spacing`, a belt's
   idlers and terminals) placed from the template's model, scaled by the record's figures named in `scales_by`.
3. **Model** (`draws: model`: flanges, terminations, transfers, chutes, bus joints, hood, clamp coupling): the
   template's model at each port a run ends on, scaled per axis to the record's `physical` (or `size`).
4. Until a template has its own model, `model_of` names an installed size whose model serves, scaled.
5. A record's own `visual`, where one is installed, may be drawn as it is in the meantime; nothing breaks.

## What changes for the loader

- `run.template`, `connector.template`: a key of kind `template` (the generated types pick them up).
- `looks_as` is retired on kit records: the template replaces it.

## Asked back

Whether the scaling rules are enough for the renderer (per-axis scale of a model; a swept profile from one diameter or
a width and height), or what else a template must say. Reply on the thread or edit this file.
