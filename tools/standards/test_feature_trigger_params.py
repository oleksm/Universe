"""Detector settings have one named, finite SI value per feature scope."""
from pathlib import Path
import copy
import unittest
from validate import feature_trigger_parameters, schema, validate

SCHEMA = str(Path(__file__).resolve().parents[2] / 'standards/PlanetSim/schema/planet-feature.schema.yaml')


class FeatureTriggerParameters(unittest.TestCase):
    def setUp(self):
        self.param = {'name': 'DROP_MIN', 'value': 10., 'unit': 'm',
                      'meaning': 'minimum resolved drop', 'open': True, 'used': False}
        self.rec = {'identity': {'key': 'planet-feature.test'}, 'trigger_params': [self.param]}
        self.sch = schema(SCHEMA)['properties']['trigger_params']

    def test_valid_finite_si_threshold(self):
        self.assertEqual(validate([self.param], self.sch, SCHEMA), [])
        self.assertEqual(feature_trigger_parameters(self.rec), [])

    def test_missing_value_or_unit_is_not_hidden_code_default(self):
        for field in ('value', 'unit'):
            rec = copy.deepcopy(self.param)
            del rec[field]
            with self.subTest(field=field):
                self.assertTrue(validate([rec], self.sch, SCHEMA))

    def test_nonfinite_boolean_string_and_unknown_field_rejected(self):
        for value in (float('nan'), float('inf'), True, '10'):
            with self.subTest(value=value):
                self.assertTrue(validate([dict(self.param, value=value)], self.sch, SCHEMA))
        self.assertTrue(validate([dict(self.param, hidden_fallback=3)], self.sch, SCHEMA))

    def test_si_unit_not_an_unconverted_label(self):
        self.param['unit'] = 'mm/yr'
        self.assertTrue(feature_trigger_parameters(self.rec))

    def test_duplicate_name_rejected_even_if_values_differ(self):
        self.rec['trigger_params'].append(dict(self.param, value=20.))
        self.assertTrue(feature_trigger_parameters(self.rec))

    def test_same_name_is_valid_in_another_feature_scope(self):
        other = copy.deepcopy(self.rec)
        other['identity']['key'] = 'planet-feature.other'
        other['trigger_params'][0]['value'] = 30.
        self.assertEqual(feature_trigger_parameters(self.rec), [])
        self.assertEqual(feature_trigger_parameters(other), [])


if __name__ == '__main__':
    unittest.main()
