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

## Next on `fso`, in this order

1. **`identity.key` on every record, and typed refs.** Proposal: the key is `<kind>.<name>`, lower
   case, words joined by `-`, kind from one closed list. Where the game has a key today the record
   takes the game's (`drive.torch.s1`, `brand.hadley`, `structure.ring.i`), so nothing is renamed
   on your side. A ref is that key as a string, and its kind is checked against what the field
   allows. Celestial bodies: `body.<system>.<name>` (`body.treistun.treistun-f`). I will add the key
   beside what is there, switch refs kind by kind, and keep the build's output the same until you
   say a loader is ready.
2. **SI throughout.** Record values in kg, m, s, W, N, K, Pa, rad. The page converts for reading.
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
