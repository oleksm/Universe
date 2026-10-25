# Registry as a service: what Blender uses, and the CLI that answers it (2026-10-24)

## Audit

- **Messages:** Blender has sent the registry 1,992 messages in all; today the Explorer alone took about 600. The topics,
  by keyword over the message texts (rough, overlapping): connectors, wires and pins; validation and QC; product records
  and packages; mass and COM; scope and what is allowed; loads and thermal; mounts and sockets; materials and densities.
  Almost all were lookups or rules the registry already held, or rules it lacked as data.
- **What Blender's code reads from the registry directly:** `tools/standards/precheck_model.py` (about 470
  references), equipment records, packages, materials, runs (cables and wires), connectors, parts, layouts, and a dozen of
  the registry's ad-hoc decision docs under `docs/ships/`.
- **What was missing as tools:** a hull's requirements (they lived in engine code), which sockets a size class offers,
  which products fit a socket, a product sheet with ports and pins, a hull check before install, and a way to say
  "this does not exist".

## Now

One CLI, `tools/standards/ship.py`, and a skill every agent can load (`freefall-registry-service`; the same text in
`docs/registry-service.md`): `requirements`, `sockets`, `modules`, `product`, `get`, `find`, `classify`, `validate`,
`decisions`, `missing`. Rules it applies are registry records - `ship-rule.base-blocks` (tested equal to the game's
`BASE_BLOCKS`) and `ship-check.socket-qualification` (which the game's import runs too).

## Rule of work

Blender looks things up itself. It messages the registry only when the CLI says something is missing; the registry makes
the record and it appears through the same CLI.

## Still open

- Decisions made today as docs (`docs/ships/*choice*.yaml`) are listed by `decisions` but are not records; recurring
  ones (wire sizing by current, connector insert layouts, gasket densities) should become records the CLI can query.
- Fit-time product clash between neighbouring sockets is not computed yet.
