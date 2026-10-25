"""schema_audit: the game's type generator's rules catch what broke planet-forge on 2026-10-24."""
import os, tempfile, unittest
import schema_audit, validate


class GeneratorRules(unittest.TestCase):
    def test_a_mixed_type_list_and_an_array_without_items_are_caught(self):
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, "standards", "X", "schema"))
            p = os.path.join(d, "standards", "X", "schema", "t.schema.yaml")
            open(p, "w").write("type: object\nproperties:\n  a: {type: [string, number]}\n  b: {type: array}\n  c: {type: [string, 'null']}\n")
            saved = schema_audit.ROOT
            schema_audit.ROOT = d
            try:
                what = " ".join(e for _, e in schema_audit.schema_rules())
            finally:
                schema_audit.ROOT = saved
                validate._schemas.pop(os.path.normpath(p), None)
        self.assertIn(".a: type list", what)
        self.assertIn(".b: an array with no items", what)
        self.assertNotIn(".c:", what)


if __name__ == "__main__":
    unittest.main()
