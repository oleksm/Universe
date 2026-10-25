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
        band = dict(want='band', **{'min': .3, 'max': .7, 'soft': .5})
        self.assertFalse(self.faults(record(band)))
        for field in ('min', 'max', 'soft'):
            bad = band.copy(); del bad[field]
            self.assertTrue(self.faults(record(bad)), field)
        for changes in ({'min': .7}, {'min': .8},
                        {'soft': 0}, {'soft': -1}, {'soft': 1.1}, {'soft': True},
                        {'min': float('nan')}, {'max': float('inf')}):
            self.assertTrue(self.faults(record(dict(band, **changes))), changes)

    def test_high_low_cannot_hide_band_fields(self):
        for want in ('high', 'low'):
            self.assertFalse(self.faults(record({'want': want})))
            for field in ('min', 'max', 'soft'):
                self.assertTrue(self.faults(record({'want': want, field: .5})))
        self.assertTrue(self.faults(record({})))
        self.assertTrue(self.faults(record({'want': 'guess-from-why'})))
        rec = record({'want': 'high'}); rec['look'] = 'guess-from-prefix'
        self.assertTrue(self.faults(rec))
        del rec['look']
        self.assertTrue(self.faults(rec))

    def test_soft_band_edges_are_continuous(self):
        band = {'want': 'band', 'min': .3, 'max': .7, 'soft': .5}
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
        for p in r['planet-pattern'].values():
            p['factors'][0]['signal'] = {'field': 'planet-field.other', 'from': 0, 'to': 10}
        self.assertEqual(lod.match(r, {'factors': {'TEST': 1}, 'fields': {'other': 0}})[0]['key'], 'low')

    def test_required_unknown_zero_and_weight_exclusion(self):
        r = self.registry()
        r['planet-factor']['planet-factor.gate'] = {'source': 'GATE', 'signal': {'field': 'planet-field.gate', 'from': 0, 'to': 1}}
        p = r['planet-pattern']['high']
        p['factors'].append({'factor': 'planet-factor.gate', 'weight': 100, 'want': 'high', 'required': True})
        for value in (None, float('nan'), float('inf'), True):
            row = next(x for x in lod.match(r, {'fields': {'test': 9, 'gate': value}}) if x['key'] == 'high')
            self.assertEqual((row['state'], row['score']), ('unknown', 0))
        self.assertNotIn('high', [x['key'] for x in lod.match(r, {'fields': {'test': 9, 'gate': 0}})])
        row = next(x for x in lod.match(r, {'fields': {'test': 9, 'gate': .1}}) if x['key'] == 'high')
        self.assertEqual(row['score'], .9)

    def test_si_conditions_and_band(self):
        r = self.registry()
        r['planet-pattern']['high']['when'] = [{'field': 'planet-field.temp', 'min': 273.15, 'max': 283.15}]
        for t in (273.15, 283.15):
            self.assertEqual(next(x for x in lod.match(r, {'fields': {'test': 9, 'temp': t}}) if x['key'] == 'high')['score'], .9)
        self.assertNotIn('high', [x['key'] for x in lod.match(r, {'fields': {'temp': 273}})])
        self.assertEqual(next(x for x in lod.match(r, {}) if x['key'] == 'high')['state'], 'unknown')
        e = {'factor': 'planet-factor.test', 'weight': 1, 'want': 'band', 'min': 3, 'max': 7, 'soft': .5}
        self.assertAlmostEqual(lod.entry_score(r, e, {'fields': {'test': 2}}), .5)
        self.assertAlmostEqual(lod.entry_score(r, e, {'fields': {'test': 5}}), 1)

    def test_directed_classes_are_not_ordered_codes(self):
        r = self.registry()
        r['planet-factor']['planet-factor.test'].update(kind='categorical',
            signal={'field': 'planet-field.rock'}, class_map=[{'key': 'limestone', 'codes': [8]}, {'key': 'clastic', 'codes': [9]}])
        e = {'factor': 'planet-factor.test', 'weight': 1, 'want': 'in', 'classes': ['limestone']}
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 8}}), 1)
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 9}}), 0)
        e['want'] = 'not-in'
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 9}}), 1)
        for value in (None, -1, 8.5, 999, float('nan'), True):
            self.assertIsNone(lod.entry_score(r, e, {'fields': {'rock': value}}))
        e['unresolved'] = True
        self.assertIsNone(lod.entry_score(r, e, {'fields': {'rock': 8}}))

    def test_ambiguous_classes_cannot_pass_or_become_known_absence(self):
        r = self.registry()
        r['planet-factor']['planet-factor.test'].update(kind='categorical', signal={'field': 'planet-field.rock'},
            class_map=[{'key': 'limestone', 'codes': [8]}, {'key': 'clastic-unit', 'codes': [9]},
                       {'key': 'fine', 'codes': [9], 'unresolved': True},
                       {'key': 'evaporite', 'codes': [], 'unresolved': True}])
        e = {'factor': 'planet-factor.test', 'weight': 1, 'want': 'in', 'classes': ['fine']}
        for want in ('in', 'not-in'):
            e['want'] = want
            self.assertIsNone(lod.entry_score(r, e, {'fields': {'rock': 9}}))
            e['classes'] = ['evaporite']
            self.assertIsNone(lod.entry_score(r, e, {'fields': {'rock': 8}}))
            e['classes'] = ['fine']
        e.update(want='in', classes=['limestone', 'fine'])
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 8}}), 1)
        self.assertIsNone(lod.entry_score(r, e, {'fields': {'rock': 9}}))

    def test_class_split_by_a_second_field(self):
        r = self.registry()
        r['planet-factor']['planet-factor.test'].update(kind='categorical', signal={'field': 'planet-field.rock'},
            class_map=[{'key': 'fine', 'codes': [9], 'and': [{'field': 'planet-field.grain', 'codes': [1], 'possible': [0]}]},
                       {'key': 'coarse', 'codes': [9], 'possible': [11], 'and': [{'field': 'planet-field.grain', 'codes': [2], 'possible': [0]}]}])
        e = {'factor': 'planet-factor.test', 'weight': 1, 'want': 'in', 'classes': ['coarse']}
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 9, 'grain': 2}}), 1)
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 9, 'grain': 1}}), 0)
        for sig in ({'rock': 9, 'grain': 0}, {'rock': 9}, {'rock': 9, 'grain': 1.5}, {'rock': 11, 'grain': 2}):
            self.assertIsNone(lod.entry_score(r, e, {'fields': sig}))
        e['want'] = 'not-in'
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 9, 'grain': 1}}), 1)
        e.update(want='in', classes=['fine', 'coarse'])
        self.assertEqual(lod.entry_score(r, e, {'fields': {'rock': 9, 'grain': 1}}), 1)

    def test_gate_only_or_trigger_owned_patterns_cannot_place(self):
        r = self.registry()
        p = r['planet-pattern']['high']
        p['factors'][0]['required'] = True
        self.assertEqual(next(x for x in lod.match(r, {'fields': {'test': 9}}) if x['key'] == 'high')['score'], 0)
        p['factors'][0].pop('required')
        p['trigger_rule'] = 'planet-feature.test'
        row = next(x for x in lod.match(r, {'fields': {'test': 9}}) if x['key'] == 'high')
        self.assertEqual((row['state'], row['score']), ('unknown', 0))
        p.pop('trigger_rule')
        r['planet-factor']['planet-factor.test']['reading'] = {'status': 'unavailable'}
        self.assertEqual([x['score'] for x in lod.match(r, {'factors': {'TEST': 1}, 'fields': {'test': 9}})], [0, 0])

    def test_time_uses_form_age_never_crust_age(self):
        r = self.registry()
        r['planet-factor']['planet-factor.time'] = {'source': 'TIME'}
        e = {'factor': 'planet-factor.time', 'weight': 1, 'want': 'high', 'tau_relax_s': 100}
        self.assertIsNone(lod.entry_score(r, e, {'fields': {'crust-age': 0}}))
        self.assertEqual(lod.entry_score(r, e, {'form_age_s': 0}), 1)
        self.assertAlmostEqual(lod.entry_score(r, e, {'form_age_s': 100}), .36787944117)
        self.assertIsNone(lod.entry_score(r, e, {'form_age_s': -1}))
        self.assertTrue(self.faults(record({'want': 'in'})))
        self.assertTrue(self.faults(record({'want': 'high', 'tau_relax_s': 100})))
        self.assertTrue(self.faults(record({'want': 'high', 'required': True})))

    def test_generated_matcher_executes_same_band_and_missing_semantics(self):
        generated = {}
        exec(compile(planet_lod_module.render(), '<generated planet module>', 'exec'), generated)
        r = self.registry()
        r['planet-pattern']['band'] = record({'want': 'band', 'min': .3, 'max': .7, 'soft': .5})
        for sig in ({}, {'fields': {'test': 2}}, {'fields': {'test': 5}}):
            self.assertEqual(generated['match'](r, sig), lod.match(r, sig))


if __name__ == '__main__':
    unittest.main()
