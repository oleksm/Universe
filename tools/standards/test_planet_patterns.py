"""Preference semantics and invalid band contracts shared by registry/generated matcher."""
from pathlib import Path
import unittest

import planet_lod as lod
import planet_lod_module
from validate import schema, validate, pattern_preferences

SCHEMA = str(Path(__file__).resolve().parents[2] / 'standards/PlanetSim/schema/planet-pattern.schema.yaml')


def record(factor):
    return {'identity': {'key': 'planet-pattern.test', 'name': 'Test', 'about': 'Test'},
            'family': 'fluvial', 'look': 'dendritic',
            'factors': [dict(factor='planet-factor.test', weight=1, **factor)]}


class PatternPreferences(unittest.TestCase):
    def faults(self, rec):
        errors = validate(rec, schema(SCHEMA), SCHEMA)
        return errors or pattern_preferences(rec)

    def test_band_requires_valid_complete_bounds(self):
        band = dict(want='band', **{'from': .3, 'to': .7, 'soft': .5})
        self.assertFalse(self.faults(record(band)))
        for field in ('from', 'to', 'soft'):
            bad = band.copy(); del bad[field]
            self.assertTrue(self.faults(record(bad)), field)
        for changes in ({'from': .7}, {'from': .8}, {'from': -.1}, {'to': 1.1},
                        {'soft': 0}, {'soft': -1}, {'soft': 1.1}, {'soft': True},
                        {'from': float('nan')}, {'to': float('inf')}):
            self.assertTrue(self.faults(record(dict(band, **changes))), changes)

    def test_high_low_cannot_hide_band_fields(self):
        for want in ('high', 'low'):
            self.assertFalse(self.faults(record({'want': want})))
            for field in ('from', 'to', 'soft'):
                self.assertTrue(self.faults(record({'want': want, field: .5})))
        self.assertTrue(self.faults(record({})))
        self.assertTrue(self.faults(record({'want': 'guess-from-why'})))
        rec = record({'want': 'high'}); rec['look'] = 'guess-from-prefix'
        self.assertTrue(self.faults(rec))
        del rec['look']
        self.assertTrue(self.faults(rec))

    def test_soft_band_edges_are_continuous(self):
        band = {'want': 'band', 'from': .3, 'to': .7, 'soft': .5}
        for value, expected in [(0, 0), (.1, 0), (.2, .5), (.3, 1), (.5, 1),
                                (.7, 1), (.8, .5), (.9, 0), (1, 0)]:
            self.assertAlmostEqual(lod.preference(band, value), expected)
        for edge in (.1, .3, .7, .9):
            self.assertLess(abs(lod.preference(band, edge+1e-8)-lod.preference(band, edge-1e-8)), 1e-6)

    def registry(self):
        return {'planet-factor': {'planet-factor.test': {'source': 'TEST',
                    'signal': {'field': 'planet-field.test', 'from': 0, 'to': 10}}},
                'planet-pattern': {want: record({'want': want}) for want in ('high', 'low')}}

    def test_preferences_change_ranking_without_reading_prose(self):
        r = self.registry()
        r['planet-pattern']['high']['factors'][0]['why'] = 'low flat quiet'
        self.assertEqual(lod.match(r, {'fields': {'test': 9}})[0]['key'], 'high')
        self.assertEqual(lod.match(r, {'fields': {'test': 1}})[0]['key'], 'low')
        self.assertEqual(lod.match(r, {'factors': {'TEST': .9}})[0]['key'], 'high')
        self.assertAlmostEqual(lod.match(r, {'fields': {'test': 1}})[0]['score'], .9)

    def test_missing_is_not_known_low(self):
        r = self.registry()
        self.assertEqual([x['score'] for x in lod.match(r, {})], [0, 0])
        self.assertEqual(lod.match(r, {'fields': {'test': 0}})[0]['score'], 1)
        r['planet-factor']['planet-factor.test']['signal'] = {'landmarks': ['planet-landmark.a']}
        self.assertEqual([x['score'] for x in lod.match(r, {})], [0, 0])
        self.assertEqual(lod.match(r, {'landmarks': []})[0]['key'], 'low')

    def test_feature_signal_requires_radius_and_excludes_mixed_modes(self):
        signal = {'features': ['planet-feature.water-gap'], 'radius_m': 25000}
        self.assertFalse(self.faults(record({'want': 'high', 'signal': signal})))
        invalid = [dict(signal, radius_m=x) for x in (0, -1, True, float('inf'))]
        invalid += [{'features': signal['features']}, {'radius_m': 3},
                    dict(signal, features=[]), dict(signal, features=signal['features']*2),
                    dict(signal, landmarks=['planet-landmark.water-gap'])]
        for sig in invalid:
            with self.subTest(sig=sig):
                self.assertTrue(self.faults(record({'want': 'high', 'signal': sig})))

    def test_entry_override_does_not_leak_global_or_given_strength(self):
        r = self.registry()
        for p in r['planet-pattern'].values():
            p['factors'][0]['signal'] = {'features': ['planet-feature.water-gap'], 'radius_m': 25000}
        for signature in ({}, {'factors': {'TEST': 1}}, {'fields': {'test': 10}}, {'landmarks': []}):
            self.assertEqual([x['score'] for x in lod.match(r, signature)], [0, 0])
        # A known override field is evaluated instead of global TEST/field.test.
        for p in r['planet-pattern'].values():
            p['factors'][0]['signal'] = {'field': 'planet-field.other', 'from': 0, 'to': 10}
        self.assertEqual(lod.match(r, {'factors': {'TEST': 1}, 'fields': {'other': 0}})[0]['key'], 'low')

    def test_generated_matcher_executes_same_band_and_missing_semantics(self):
        # Generate in memory, rather than depending on a previously built module.
        generated = {}
        exec(compile(planet_lod_module.render(), '<generated planet module>', 'exec'), generated)
        r = self.registry()
        r['planet-pattern']['band'] = record({'want': 'band', 'from': .3, 'to': .7, 'soft': .5})
        for sig in ({}, {'fields': {'test': 2}}, {'fields': {'test': 5}}):
            self.assertEqual(generated['match'](r, sig), lod.match(r, sig))


if __name__ == '__main__':
    unittest.main()
