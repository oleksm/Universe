"""ship.py: requirements the game reads from the record, and validate's requirement criteria."""
import os, unittest
import yaml
from pathlib import Path
import ship

GAME = Path(os.path.expanduser("~/git/universe/crates/core/world/src/modules.rs"))


class ShipCli(unittest.TestCase):
    def test_the_game_takes_its_base_blocks_from_this_record(self):
        if not GAME.is_file():
            self.skipTest("no game checkout")
        src = GAME.read_text()
        self.assertIn('"ship-rule.base-blocks"', src)
        self.assertNotRegex(src, r"BASE_BLOCKS: \[SlotKind")
        self.assertTrue(any(b["required"] for b in ship.blocks()))

    def test_every_block_kind_is_a_slot_kind_once(self):
        kinds = [k for b in ship.blocks() for k in b["kinds"]]
        self.assertEqual(len(kinds), len(set(kinds)))


    def test_an_engine_slot_stands_in_by_its_role(self):
        lift = next(b for b in ship.blocks() if b["need"] == "lift")
        self.assertTrue(ship.offered([{"name": "l", "kind": "engine", "role": "lift"}], lift))
        self.assertFalse(ship.offered([{"name": "m", "kind": "engine", "role": "main"}], lift))


class ServiceCommands(unittest.TestCase):
    def test_modules_for_a_mount_use_the_games_fit_rule(self):
        import io, contextlib, json
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ship.modules({"--mount": "mount.engine-s1c", "--json": True})
        keys = {r["key"] for r in json.loads(out.getvalue())}
        self.assertIn("equipment.engine.cg.s1", keys)
        self.assertNotIn("equipment.engine.ft.s1", keys)

    def test_missing_prints_one_request(self):
        import io, contextlib
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ship.missing("a thing", {"--for": "equipment.x"})
        self.assertIn("agentmsg send registry task", out.getvalue())


class ShipRecords(unittest.TestCase):
    def test_explorer_hull_is_bare(self):
        """Owner 2026-10-24: the hull is hull and sockets; a workable fit is a ship record."""
        rec = yaml.safe_load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../standards/SFO/metadata/hulls/explorer-study.yaml")))
        self.assertEqual(rec.get("fit", []), [])
        self.assertEqual(rec.get("thrusters", []), [])

    def test_path_defines_key(self):
        from validate import key_of
        self.assertEqual(key_of(os.path.join("SFO", "metadata", "ship-checks", "shipyard-fit.yaml"), {}), "ship-check.shipyard-fit")


if __name__ == "__main__":
    unittest.main()
