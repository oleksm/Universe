Planet patterns now state their factor preference (`want: high|low|band`) and drawing archetype (`look`); consumers no longer need to infer these from prose or key prefixes. A band uses normalized `from < to` and required `soft`: score 1 inside, with linear outside ramps each `soft * (to - from)` wide. Missing signals score zero rather than becoming a satisfied low preference.

The catalogue adds 66 proposed feature kinds (241 total), preserving supplied rough sizes as review-required estimates and deriving their numeric-band ladders. Shapes, measured examples and simulator triggers are not invented to fill gaps. The 185 initial pattern mappings are draft values requiring review.

Registry matcher and generated Python module honor the new fields. Validation rejects missing, reversed, nonfinite or zero-width bands and band fields attached to high/low preferences. See `docs/diagnostics/planet-pattern-keys-20261022.yaml` for migration provenance and checks.
