"""ship.py: requirements held to the game's list, and validate's requirement criteria."""
import os, re, unittest
from pathlib import Path
import ship

GAME = Path(os.path.expanduser("~/git/universe/crates/core/world/src/modules.rs"))
CAMEL = {"LifeSupport": "life_support"}


class ShipCli(unittest.TestCase):
    def test_required_blocks_are_the_games_base_blocks(self):
        required = {k for b in ship.blocks() if b["required"] for k in b["kinds"]}
        if not GAME.is_file():
            self.skipTest("no game checkout")
        line = re.search(r"BASE_BLOCKS: \[SlotKind; \d+\] = \[(.*?)\];", GAME.read_text()).group(1)
        game = {CAMEL.get(n, n.lower()) for n in re.findall(r"SlotKind::(\w+)", line)}
        self.assertEqual(required, game)

    def test_every_block_kind_is_a_slot_kind_once(self):
        kinds = [k for b in ship.blocks() for k in b["kinds"]]
        self.assertEqual(len(kinds), len(set(kinds)))



class ServiceCommands(unittest.TestCase):
    def test_modules_for_a_mount_use_the_games_fit_rule(self):
        import io, contextlib, json
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ship.modules({"--mount": "mount.engine-s1c", "--json": True})
        keys = {r["key"] for r in json.loads(out.getvalue())}
        self.assertIn("equipment.engine.cg.s1", keys)
        self.assertNotIn("equipment.engine.ft.s1", keys)   # 30 t, 6 m: past the compact envelope

    def test_missing_prints_one_request(self):
        import io, contextlib
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            ship.missing("a thing", {"--for": "equipment.x"})
        self.assertIn("agentmsg send registry task", out.getvalue())


if __name__ == "__main__":
    unittest.main()
