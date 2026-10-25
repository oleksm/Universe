"""Per-instance rest scope must bind to the fitted product's actual motion package."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from validate import hull_fit_poses, schema, validate


class HullFitPose(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.folder = self.home / 'models/engine/v1'
        self.folder.mkdir(parents=True)
        self.motion = b'{"format":"freefall-motion/1"}'
        (self.folder / 'motion.json').write_bytes(self.motion)
        self.manifest = {'key': 'equipment.engine.test', 'motion': {'file': 'motion.json'},
                         'files': {'motion.json': {'sha256': hashlib.sha256(self.motion).hexdigest(),
                                                   'size': len(self.motion)}}}
        self.equipment = {'visual': {'path': 'models/engine/v1'}}
        self.hull = {'fit': [{'slot': 'engine_port', 'item': 'equipment.engine.test', 'pose': 'rest'}]}
        self.pin()

    def pin(self):
        raw = json.dumps(self.manifest).encode()
        (self.folder / 'manifest.json').write_bytes(raw)
        self.equipment['visual']['manifest_sha256'] = hashlib.sha256(raw).hexdigest()

    def errors(self):
        return hull_fit_poses([('hull.test', 'hull.yaml', self.hull, {}),
                               ('equipment.engine.test', 'engine.yaml', self.equipment, {})], self.home)

    def test_rest_and_default_are_per_instance(self):
        self.hull['fit'].append({'slot': 'engine_starboard', 'item': 'equipment.engine.test'})
        self.assertEqual(self.errors(), [])
        del self.hull['fit'][0]['pose']
        self.equipment.clear()
        self.assertEqual(self.errors(), [])

    def test_static_visual_cannot_be_opted_in(self):
        del self.manifest['motion']
        self.pin()
        self.assertIn('no motion', self.errors()[0][1])

    def test_missing_and_stale_motion_fail(self):
        (self.folder / 'motion.json').unlink()
        self.assertTrue(self.errors())
        (self.folder / 'motion.json').write_bytes(b'changed')
        self.assertIn('hash mismatch', self.errors()[0][1])

    def test_unpinned_or_different_product_fails(self):
        self.equipment['visual']['manifest_sha256'] = '0' * 64
        self.assertIn('manifest hash mismatch', self.errors()[0][1])
        self.manifest['key'] = 'equipment.engine.other'
        self.pin()
        self.assertIn('different product', self.errors()[0][1])

    def test_missing_visual_and_malformed_inventory_fail(self):
        self.equipment.clear()
        self.assertTrue(self.errors())
        self.equipment['visual'] = {'path': 'models/engine/v1'}
        self.manifest['files'] = []
        self.pin()
        self.assertIn('inventory', self.errors()[0][1])

    def test_malformed_fit_collection_is_left_to_schema(self):
        for value in (True, 42, "not a list", {}):
            self.hull["fit"] = value
            self.assertEqual(self.errors(), [])

    def test_pose_enum(self):
        path = str(Path(__file__).resolve().parents[2] / 'standards/SFO/schema/hull.schema.yaml')
        fit_schema = schema(path)['properties']['fit']
        self.assertEqual(validate(self.hull['fit'], fit_schema, path), [])
        for value in ('animated', False, 0, None):
            with self.subTest(value=value):
                self.hull['fit'][0]['pose'] = value
                self.assertTrue(validate(self.hull['fit'], fit_schema, path))

    def test_package_escape_rejected(self):
        self.manifest['motion']['file'] = '../outside.json'
        self.pin()
        self.assertIn('escapes', self.errors()[0][1])


if __name__ == '__main__':
    unittest.main()
