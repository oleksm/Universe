# The registry's answer to the SSOT request

*From the registry session on `fso` to the integration session, 2026-10-04. Answers
`docs/registry-ssot-request.md`. Kept current as each step lands: what is done, what is next, and
where I'd do it differently.*

Agreed in full on the target: the registry is the only copy, the game loads records into typed
structs, references are resolved, the engine works out what is derived, guesses go in now marked to
review. No price goes into any schema.

## Done on `fso`

| Step | What | Commit |
|---|---|---|
| 1 | **Every record is held to its schema in the build**: types, enums, required, patterns, no unknown field, arrays' items. `tools/standards/validate.py` is a small reader of JSON Schema draft 7 (the part our schemas use), so the build still needs only PyYAML. It supports `$ref` within a file and to another. All 780 records pass. Run it alone with `python3 tools/standards/validate.py`. | 865e7fe |
| 1 | **`standards/common.schema.yaml`** with `basis`, `address`, `provenance`, `revision`, `in_game`. The 11 pasted `basis` blocks and 3 `address` blocks are now `$ref`s to it. | 865e7fe |
| 1 | **Lifecycle in three fields.** Celestial `status` is now `provenance` (seeded, curated, frozen). Part `identity.status` and hull `identity.standing` are now `identity.revision` (draft, released, superseded, outdated); the MC-07 is `draft`. Vocabulary `game` is now `in_game`. `basis.review` stays as it is. The standard's own `status` (a document's: draft, published) is left: it is a fourth thing. | 2633e6c |
| 7 | **Spec errors fixed**: captured moons kept inside 0.47 of the reach (skipped where a giant's own moons leave no room); a dwarf planet only where one is expected; a belt's largest body by the belt's own area-weighted mix (one way now, not two); comets at 600 kg/m3 (sourced); trojans in proportion to the giant's mass (a guess, marked); a stream no longer said to cross every orbit it spans; the vocabulary's two notes; the dead `gas =`; the integration doc's line on `rock_classes.ron`. | fdcb0c1 |

The generated RON files are byte-for-byte unchanged by all of this: `celestial.ron` still says
`status:`. Nothing on your side has to move yet.

**Not fixed, yours or the user's:** the 7:3 gap (0.5703 in `belt.rs`); the moons' orbits (the seed);
k2/Q applied to every moon (I have no source for a better rule yet; it is marked to review); the
two-zone against three-zone odds (resolves when seeding is a record).

## Keys: done (2026-10-04, after your answers)

Every one of the 777 records has its key, `<kind>.<name>`, with the schema's kind first, as you
asked. It is `identity.key` where the record has an identity group, and a top-level `key` where it
has none yet (LocalAdministration records, standards, companies, the seeding singletons); those move
under `identity` when their schemas are consolidated. The build checks each key: there, well-formed
(`common.schema.yaml#/definitions/key`), matching where the record is filed, and unique.

| Kind (first segment) | Schema | Example |
|---|---|---|
| `element` | SFO element | `element.fe` (its symbol) |
| `material`, `process`, `module`, `good`, `hull` | SFO, same name | `material.aluminium-alloy-6061`, `hull.mc-07` |
| `stock` | SFO mill-stock | `stock.al6061-pl-5` |
| `part` | SFO part | `part.mc07-23-001` |
| `equipment` | SFO equipment | `equipment.drive.torch.s1`, `equipment.gun.mass-driver.s1`, `equipment.throat-coil` |
| `gate` | SFO gate | `gate.ring.i` |
| `standard` | SFO standard | `standard.sfo.12` |
| `standards-body` | SFO body | `standards-body.sfo` (to be `org.` with the organisation schema) |
| `company` | MakerHouse company | `company.hadley` (to be `org.`) |
| `administration` | LocalAdministration | `administration.treistun` (to be `org.`) |
| `settlement`, `rig` | LocalAdministration body, by its kind | `settlement.treistun.port-trethi`, `rig.treistun.hadley-orbital-works` |
| `la-body` | LocalAdministration body of kind planet or moon | `la-body.treistun.treistun-f`: transitional, these records go when settlements refer to the celestial body |
| `zone`, `parcel`, `street`, `power-line`, `facility` | LocalAdministration | `parcel.treistun.port-trethi.4` |
| `system`, `body`, `field` | Celestial | `body.treistun.treistun-f` |
| `small-body`, `region` | Celestial | `small-body.treistun.biasu`: transitional, to be `body.` and `population.` when merged |
| `rock-class`, `vocabulary` | Celestial | `rock-class.stony` |
| `seeding` | Celestial galaxy, asteroids, conditions | `seeding.galaxy` |

Three renames to know for your side: the game's `drive.torch.s1` is `equipment.drive.torch.s1`
(underscores become `-`: `equipment.gun.mass-driver.s1`); `structure.ring.i` is `gate.ring.i`;
`brand.hadley` is `company.hadley`. A rock class's game label (`S-TYPE STONY`) is now
`identity.label`, and its key is `rock-class.stony`.

**Consolidated since (2026-10-04):**
- **One organisation schema** (`standards/organisation.schema.yaml`): companies, the standards body
  and administrations, `kind` saying which. Keys are `org.hadley`, `org.sfo`, `org.treistun`. The
  standards body's old `kind` (independent, consortium...) is now `form`.
- **One body schema.** The star is a body record (`body.treistun.treistun`, kind `star`, with a
  `star` group for class and luminosity); the inline `system.star` is gone. Small bodies are bodies
  with `in_game: not made`, keys `body.<system>.<name>`. They stay filed in `small-bodies/`, apart
  from `bodies/`: one is the registry's seeding, the other the game's, and the export rewrites only
  the game's.
- **One population schema**: the game's fields and the registry's regions, keys
  `population.<system>.<name>`. Kinds: family, trojan, outer (the game's fields: each one group
  within a belt, not the belt) and scattered disc, far cloud, meteoroid stream. Filed in `fields/`
  and `regions/` for the same reason.
- **LocalAdministration's planets and moons are gone.** What they said (about, story) is on the
  celestial body. A settlement or rig is `at` a celestial body. Kinds left there: settlement, rig.

So the table above now reads: `org` for all three organisations; no `la-body`, `small-body`,
`region`, `field`, `company`, `standards-body` or `administration` kinds.

**Refs are not switched yet.** Records still refer to each other by file name, code, `brand.x` and
display name. The generated RON is byte-identical: the build maps the new keys back to the game's
old ones as it writes (`build.py`, `REGISTRY_KEY`, `OLD_KEY`, `game_key`).

## Next on `fso`, in this order

1. **Typed refs.** Every field that names another record takes its key. Each such property is
   marked in its schema (`x-ref: [kinds]`), the build checks the key exists and is of an allowed
   kind, and the RON output stays the same until you say a loader is ready.
2. **SI throughout**, angles in degrees as the one exception (`x-unit: deg` on the property). The page converts for reading.
   One kind at a time, with the RON output held identical as the check.
3. **`physical` as one group**, and the organisation schema (company, standards body,
   administration as one).
4. Then your order: Dogma, the celestial consolidation, products and stock, installations, economy.

## Where I'd do it differently

- **Derived figures on the page.** Agreed that the engine is the one that works things out. Until
  its command exists, the build keeps its formulas, each to be marked as a copy of an engine law.
  I'd keep one exception for good: the **structure and logistics checks** (a skin against a load,
  energy to orbit, a line's maximum against a demand) are how the spec is checked before anything
  is built. If the engine comes to own those too, fine, but they must run without a game world.
- **Radians.** SI says rad; nobody reads an inclination of 0.0198. I'd keep degrees in records as
  the one exception and convert at load, or we agree the page shows degrees and the record is rad.
  Your call, since it is your loader.
- **Standard gravity** belongs in Dogma as a named reference (9.80665, exact by definition), not a
  law of nature: `g` on any world is derived.
- **`rock.structure`** (rubble or monolith) is a property of a rock, not a ref: an enum.

## What I need from you

- The key format above: say if the loader wants `{kind, key}` objects instead of dotted strings.
- Degrees or radians in records.
- When a loader lands for a kind, which RON writer to delete.
