"""Regression checks for omissions a final-recipe-only audit cannot detect."""
import copy
import unittest
import yaml
from production_chain import audit, ROOT


def record(key, **extra):
    return {"identity": {"key": key, "name": key}, **extra}


def recipe(out, *inputs, outputs=()):
    return {"makes": out, "inputs": [{"item": i, "quantity": 1} for i in inputs],
            "outputs": [{"item": i, "quantity": 1} for i in outputs]}


class ProductionChainTests(unittest.TestCase):
    def setUp(self):
        self.records = {
            "element.c": record("element.c"),
            "package.box": record("package.box", physical={"mass": 2}),
            "stock.box": record("stock.box", supplies={"item": "package.box"}, physical={"mass": 2}),
            "stock.product": record("stock.product", package="package.box"),
            "module.shop": record("module.shop", recipes=[recipe("stock.box", "element.c")]),
        }

    def product_issues(self):
        return next(r["issues"] for r in audit(self.records)["rows"] if r["key"] == "stock.product")

    def test_recipe_with_unmade_ingredient_is_not_complete(self):
        self.records["module.shop"]["recipes"].append(recipe("stock.product", "good.resin"))
        self.assertTrue(self.product_issues())
        self.records["good.resin"] = record("good.resin")
        self.records["module.shop"]["recipes"].append(recipe("good.resin", "element.c"))
        self.assertFalse(audit(self.records)["issues"])

    def test_cycles_need_a_supplied_alternative_and_byproducts_need_inputs(self):
        rs = self.records["module.shop"]["recipes"]
        rs += [recipe("good.a", "good.b", outputs=("stock.product",)), recipe("good.b", "good.a")]
        self.assertTrue(self.product_issues())
        rs.append(recipe("good.a", "element.c"))
        self.assertFalse(self.product_issues())

    def test_package_must_exist_be_unique_and_match_tare(self):
        self.records["module.shop"]["recipes"].append(recipe("stock.product", "element.c"))
        self.assertFalse(audit(self.records)["issues"])
        self.records["stock.box"]["physical"]["mass"] = 3
        self.assertTrue(any("tare" in i for i in audit(self.records)["issues"]))
        self.records["stock.box2"] = copy.deepcopy(self.records["stock.box"])
        self.assertTrue(any("2 empty-stock" in i for i in audit(self.records)["issues"]))
        del self.records["stock.box"], self.records["stock.box2"]
        self.assertTrue(any("0 empty-stock" in i for i in audit(self.records)["issues"]))

    def test_compound_product_does_not_supply_its_constituents(self):
        self.records["stock.box"]["made_from"] = [{"item": "material.resin"}, {"item": "element.c"}]
        self.records["module.shop"]["recipes"].append(recipe("stock.product", "material.resin"))
        self.assertTrue(self.product_issues())
        self.assertNotIn("material.resin", audit(self.records)["available"])

    def test_new_manufacturing_budgets_conserve_mass(self):
        for slug in ("specialty-chemical-works", "packaging-works", "factory-works"):
            d = yaml.safe_load((ROOT / "standards/SFO/metadata/modules" / (slug + ".yaml")).read_text())
            for r in d["recipes"]:
                with self.subTest(product=r["makes"]):
                    incoming = sum(i["quantity"] for i in r["inputs"])
                    outgoing = 1 + sum(o["quantity"] for o in r.get("outputs", []))
                    self.assertAlmostEqual(incoming, outgoing, delta=1e-6)

    def test_works_mass_is_the_sum_of_construction_parts(self):
        meta = ROOT / "standards/SFO/metadata"
        factory = yaml.safe_load((meta / "modules/factory-works.yaml").read_text())
        for r in factory["recipes"]:
            slug = r["makes"].split(".", 1)[1]
            works = yaml.safe_load((meta / "modules" / (slug + ".yaml")).read_text())
            parts = [yaml.safe_load(p.read_text()) for p in (meta / "parts" / slug).glob("*.yaml")]
            with self.subTest(works=slug):
                self.assertTrue(parts)
                self.assertEqual(works["physical"]["mass"], sum(p["physical"]["mass"] * p.get("fit", {}).get("count", 1) for p in parts))


if __name__ == "__main__":
    unittest.main()
