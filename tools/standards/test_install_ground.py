"""Ground snapshots preserve source bytes, survey state and immutable provenance."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import yaml
import install_ground as installer
import set_ground


class GroundInstall(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "view"
        self.source.mkdir()
        self.store = self.root / "store"
        self.store.mkdir()
        self.key = "body.test.world"
        self.record = self.root / "standards/Celestial/metadata/systems/test/bodies/world.yaml"
        self.record.parent.mkdir(parents=True)
        self.original = {"identity": {"key": self.key}, "survey": {"world_id": "TRE3", "manifest_sha256": "a" * 64},
                         "physical": {"radius": 6281370}, "surface": {"mean_temperature": 216.2},
                         "ground": {"format": "planet-unfold-tiles/1", "path": "old"}}
        self.record.write_text(yaml.safe_dump(self.original))
        (self.source / "pyramid").mkdir()
        for name in ("lod.json", "dir.f32", "z.f32", "kind.u8", "tri.u32"):
            (self.source / name).write_bytes(b"123")
        (self.source / "info.json").write_text(json.dumps({"radius_m": 6371000, "source": "fixture"}))
        level = {"k": 0, "n": 16, "z": "z_0.f32", "rgb": "rgb_0.u8", "mask": "mask_0.u8",
                 "window": {"row0": 0, "col0": 0, "rows": 2, "cols": 3}}
        (self.source / "pyramid/pyramid.json").write_text(json.dumps({"format": "planet-forge land pyramid 1",
            "projection": {"radius_m": 6371000}, "branch": 4, "top_level": 0, "levels": [level]}))
        for name, size in (("z_0.f32", 24), ("rgb_0.u8", 18), ("mask_0.u8", 6)):
            (self.source / "pyramid" / name).write_bytes(bytes(size))
        for name in installer.EXCLUDED:
            (self.source / name).mkdir()
            (self.source / name / "comparison.bin").write_bytes(b"excluded")
        root_patch = patch.object(set_ground, "ROOT", str(self.root))
        env_patch = patch.dict(os.environ, {"UNIVERSE_WORLDS": str(self.store)})
        root_patch.start(); env_patch.start()
        self.addCleanup(root_patch.stop); self.addCleanup(env_patch.stop)

    def install(self, **options):
        result = installer.install(self.source, self.key, apply=True, **options)
        return result, self.store / result["path"]

    def test_dry_run_does_not_write(self):
        before = self.record.read_bytes()
        result = installer.install(self.source, self.key)
        self.assertTrue(result["dry_run"])
        self.assertEqual(self.record.read_bytes(), before)
        self.assertEqual(list(self.store.iterdir()), [])

    def test_source_bytes_survey_and_exclusions(self):
        result, destination = self.install()
        manifest, digest, size = set_ground.validate_manifest(destination, self.key, "TRE3", verify_hashes=True)
        self.assertEqual(size, result["bytes"])
        self.assertEqual(digest, result["manifest_sha256"])
        for name in manifest["files"]:
            self.assertEqual((destination / name).read_bytes(), (self.source / name).read_bytes())
        for name in installer.EXCLUDED:
            self.assertFalse((destination / name).exists())
        after = yaml.safe_load(self.record.read_text())
        self.assertEqual({k: v for k, v in after.items() if k != "ground"},
                         {k: v for k, v in self.original.items() if k != "ground"})
        self.assertEqual(after["ground"]["format"], installer.FORMAT)

    def test_reinstall_reuses_exact_snapshot(self):
        first, destination = self.install()
        before = (destination / "manifest.json").stat().st_mtime_ns
        second, _ = self.install()
        self.assertTrue(second["reused"])
        self.assertEqual(first["path"], second["path"])
        self.assertEqual(first["manifest_sha256"], second["manifest_sha256"])
        self.assertEqual((destination / "manifest.json").stat().st_mtime_ns, before)

    def test_changed_source_never_overwrites_named_snapshot(self):
        first, destination = self.install(snapshot_name="fixed")
        before = self.record.read_bytes()
        (self.source / "tri.u32").write_bytes(b"new bytes")
        with self.assertRaisesRegex(ValueError, "immutable"):
            self.install(snapshot_name="fixed")
        self.assertEqual(self.record.read_bytes(), before)
        self.assertEqual((destination / "tri.u32").read_bytes(), b"123")

    def test_source_change_aborts_before_publish(self):
        inventory = installer.inventory(self.source)
        changed = copy.deepcopy(inventory)
        changed["tri.u32"] = (999, 0, 0)
        before = self.record.read_bytes()
        with patch.object(installer, "inventory", side_effect=[inventory, changed]):
            with self.assertRaisesRegex(ValueError, "source package changed"):
                self.install()
        self.assertEqual(self.record.read_bytes(), before)
        self.assertEqual(list((self.store / "worlds/TRE3/ground").iterdir()), [])

    def test_window_arrays_must_match_dimensions(self):
        (self.source / "pyramid/z_0.f32").write_bytes(b"bad")
        with self.assertRaisesRegex(ValueError, "wrong size"):
            self.install()

    def test_concurrent_body_edit_is_not_overwritten(self):
        inventory = installer.inventory
        calls = 0
        changed = copy.deepcopy(self.original)
        changed["physical"]["radius"] = 6000000

        def scan(source):
            nonlocal calls
            calls += 1
            if calls == 2:
                self.record.write_text(yaml.safe_dump(changed))
            return inventory(source)

        with patch.object(installer, "inventory", side_effect=scan):
            with self.assertRaisesRegex(ValueError, "body record changed"):
                self.install()
        self.assertEqual(yaml.safe_load(self.record.read_text()), changed)

    def test_sizes_checked_without_per_read_hashes(self):
        _, destination = self.install()
        (destination / "tri.u32").write_bytes(b"xxx")
        set_ground.validate_manifest(destination, self.key, "TRE3")
        with self.assertRaisesRegex(ValueError, "sha256"):
            set_ground.validate_manifest(destination, self.key, "TRE3", verify_hashes=True)
        (destination / "tri.u32").write_bytes(b"shortened differently")
        with self.assertRaisesRegex(ValueError, "wrong size"):
            set_ground.validate_manifest(destination, self.key, "TRE3")

    def test_wrong_body_and_world_rejected(self):
        _, destination = self.install()
        for key, world in (("body.other.world", "TRE3"), (self.key, "TRE4")):
            with self.subTest(key=key, world=world), self.assertRaises(ValueError):
                set_ground.validate_manifest(destination, key, world)
        with self.assertRaisesRegex(ValueError, "keep.*survey"):
            self.install(world="TRE4")

    def test_manifest_path_escape_rejected(self):
        _, destination = self.install()
        path = destination / "manifest.json"
        data = json.loads(path.read_text())
        data["files"]["../escape"] = {"size": 1, "sha256": "a" * 64}
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "invalid ground file"):
            set_ground.validate_manifest(destination, self.key, "TRE3")

    def test_legacy_format_still_opens(self):
        _, destination = self.install()
        path = destination / "manifest.json"
        data = json.loads(path.read_text()); data["format"] = "planet-unfold-tiles/1"
        path.write_text(json.dumps(data))
        manifest, _, _ = set_ground.validate_manifest(destination, self.key, "TRE3")
        self.assertEqual(manifest["format"], "planet-unfold-tiles/1")


if __name__ == "__main__":
    unittest.main()
