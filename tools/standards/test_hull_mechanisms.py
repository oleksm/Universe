"""Record integrity, independently of game contact/pose/route qualification."""
import copy
from pathlib import Path
import unittest

from hull_mechanisms import errors
from validate import schema, validate


def fixture():
    return {'model': 'assets/models/test.glb', 'model_sha256': 'a' * 64,
            'mechanisms': [{'key': 'ladder', 'kind': 'ladder', 'nodes': ['hinge', 'extension'],
                            'default': 'closed', 'deploys_when': 'pilot',
                            'states': [{'key': 'closed', 'poses': [{'node': n, 'kind': 'rest'} for n in ['hinge', 'extension']]},
                                       {'key': 'deployed', 'poses': [
                                           {'node': 'hinge', 'kind': 'rigid', 'translation_m': [1, 2, 3], 'rotation_xyzw': [0, 0, 1, 0]},
                                           {'node': 'extension', 'kind': 'rest'}]}],
                            'contact': {'node': 'toe', 'ground': 'required'}}],
            'exits': [{'key': 'belly', 'kind': 'climb', 'requires': [{'mechanism': 'ladder', 'state': 'deployed'}],
                       'top': 'top', 'foot': 'foot'}]}


class HullMechanisms(unittest.TestCase):
    def setUp(self):
        self.hull = fixture()
        self.path = str(Path(__file__).resolve().parents[2] / 'standards/SFO/schema/hull.schema.yaml')
        self.properties = schema(self.path)['properties']

    def schema_errors(self):
        return [e for key, value in self.hull.items() for e in validate(value, self.properties[key], self.path, key)]

    def test_complete_record_and_legacy(self):
        self.assertEqual(errors(self.hull), [])
        self.assertEqual(self.schema_errors(), [])
        self.assertEqual(errors({}), [])
        self.assertEqual(errors({'mechanisms': [], 'exits': []}), [])

    def test_exact_model_binding_required(self):
        for key in ('model', 'model_sha256'):
            hull = copy.deepcopy(self.hull)
            del hull[key]
            self.assertTrue(errors(hull))
        self.hull['model_sha256'] = 'stale'
        self.assertTrue(self.schema_errors())

    def test_duplicate_keys_and_node_owners(self):
        self.hull['mechanisms'].append(copy.deepcopy(self.hull['mechanisms'][0]))
        issues = errors(self.hull)
        self.assertTrue(any('duplicate' in x for x in issues))
        self.assertTrue(any('already owned' in x for x in issues))

    def test_default_and_automatic_states(self):
        m = self.hull['mechanisms'][0]
        m['default'] = 'missing'
        self.assertTrue(errors(self.hull))
        m['default'] = 'closed'
        m['states'][1]['key'] = 'open'
        m['deploys_when'] = 'always'
        self.assertTrue(any('requires deployed' in x for x in errors(self.hull)))

    def test_gear_requires_flight_and_landed_states(self):
        m = self.hull['mechanisms'][0]
        m['kind'] = 'gear'
        self.assertTrue(any('requires stowed' in x for x in errors(self.hull)))
        m['states'][0]['key'] = m['default'] = 'stowed'
        self.assertEqual(errors(self.hull), [])

    def test_state_and_node_names_unique(self):
        m = self.hull['mechanisms'][0]
        m['states'].append(copy.deepcopy(m['states'][0]))
        m['nodes'].append('hinge')
        self.assertGreaterEqual(sum('duplicate' in x for x in errors(self.hull)), 2)

    def test_missing_extra_and_duplicate_poses(self):
        poses = self.hull['mechanisms'][0]['states'][0]['poses']
        poses.pop()
        self.assertTrue(any('missing' in x for x in errors(self.hull)))
        poses.append({'node': 'unknown', 'kind': 'rest'})
        self.assertTrue(any('extra' in x for x in errors(self.hull)))
        poses.append(copy.deepcopy(poses[0]))
        self.assertTrue(any('duplicate' in x for x in errors(self.hull)))

    def test_quaternions_are_not_silently_normalized(self):
        pose = self.hull['mechanisms'][0]['states'][1]['poses'][0]
        for rotation in ([0, 0, 0, 0], [0, 0, 2, 0], [float('nan'), 0, 0, 1], [float('inf'), 0, 0, 1]):
            with self.subTest(rotation=rotation):
                pose['rotation_xyzw'] = rotation
                self.assertTrue(errors(self.hull))

    def test_rigid_shape_and_no_scale_or_euler(self):
        pose = self.hull['mechanisms'][0]['states'][1]['poses'][0]
        for key, value in [('scale', [1, 1, 1]), ('rotate_deg', [0, 90, 0]), ('matrix', list(range(16)))]:
            pose[key] = value
            self.assertTrue(self.schema_errors())
            del pose[key]
        del pose['translation_m']
        self.assertTrue(self.schema_errors())

    def test_nonfinite_translation_and_bad_quaternion_shape(self):
        pose = self.hull['mechanisms'][0]['states'][1]['poses'][0]
        pose['translation_m'][0] = float('nan')
        self.assertTrue(self.schema_errors())
        pose['translation_m'][0] = True
        pose['rotation_xyzw'] = [0, 0, 1]
        self.assertTrue(self.schema_errors())

    def test_exit_reference_and_required_state(self):
        req = self.hull['exits'][0]['requires'][0]
        req['mechanism'] = 'absent'
        self.assertTrue(any('no mechanism' in x for x in errors(self.hull)))
        req['mechanism'] = 'ladder'
        req['state'] = 'absent'
        self.assertTrue(any('no state' in x for x in errors(self.hull)))

    def test_conflicting_exit_requirements(self):
        self.hull['exits'][0]['requires'].append({'mechanism': 'ladder', 'state': 'closed'})
        self.assertTrue(any('duplicate' in x for x in errors(self.hull)))

    def test_exit_endpoints_and_duplicate_key(self):
        exit = self.hull['exits'][0]
        exit['foot'] = exit['top']
        self.assertTrue(any('distinct' in x for x in errors(self.hull)))
        del exit['foot']
        self.assertTrue(any('requires foot' in x for x in errors(self.hull)))
        exit['kind'] = 'walk'  # walk requires an entry marker, not an invented teleport foot
        self.assertEqual(errors(self.hull), [])
        del exit['top']
        self.assertTrue(self.schema_errors())
        self.hull['exits'].append(copy.deepcopy(exit))
        self.assertTrue(any('duplicate' in x for x in errors(self.hull)))

    def test_schema_handles_malformed_structures_without_checker_crash(self):
        for value in (True, 1, 'wrong', {}):
            with self.subTest(value=value):
                self.hull = fixture()
                self.hull['mechanisms'] = value
                self.assertEqual(errors(self.hull), [])
                self.assertTrue(self.schema_errors())
        self.hull = fixture()
        self.hull['mechanisms'][0]['default'] = []
        self.assertTrue(errors(self.hull))


if __name__ == '__main__':
    unittest.main()
