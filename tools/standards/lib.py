"""Shared by the registry's tools: the rules every script and report must agree on. Run nothing here; import it.

    import sys, os; sys.path.insert(0, os.path.dirname(__file__)); from lib import jet_heat, plant_waste
"""
import hashlib, os

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
TREE = os.path.join(ROOT, "standards")
JET_KINDS = ("engine", "drive", "lift", "thrusters")
OLD_HEAT_TO_HULL = 1e-6   # the first guess the old drive, lift and thruster records carry when they say nothing (SFO 22 engines must say)


def jet_power(fn):
    """W in the jet of a propulsion function: half the thrust times the exhaust speed."""
    return 0.5 * fn.get("thrust", 0.0) * fn.get("exhaust", 0.0)


def jet_heat(fn, nozzles=1):
    """W that comes aboard from a propulsion function's jet (SFO 22): the jet's power times the record's heat_to_hull, which
    an engine record derives from its isotropic loss, the hull's share of the sphere and its shadow shield. One rule for
    mounts, budgets and the studio."""
    if fn.get("kind") not in JET_KINDS:
        return 0.0
    return jet_power(fn) * nozzles * fn.get("heat_to_hull", OLD_HEAT_TO_HULL)


def plant_waste(fn):
    """W a power plant rejects: its output times (1 / efficiency - 1)."""
    if fn.get("kind") != "power_plant":
        return 0.0
    return fn["output"] * (1.0 / fn["efficiency"] - 1.0)


def device_heat(fn, nozzles=1):
    """W aboard from one device's own working: a plant's waste or a jet's share."""
    return plant_waste(fn) + jet_heat(fn, nozzles)


q = lambda s: '"' + s.replace('"', '\\"') + '"'   # a YAML double-quoted scalar


def edit(path, fn):
    """Rewrite `path` through `fn(text) -> text`; a YAML file is parsed before it is written back."""
    import yaml
    s = open(path, encoding="utf-8").read(); t = fn(s)
    if t != s:
        if path.endswith(".yaml"):
            yaml.safe_load(t)
        open(path, "w", encoding="utf-8").write(t)


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


# ---- parts derived from an equipment record's list (SFO 12): one rule for the build, the registry reader and the check.

def _num(v):
    return repr(v) if isinstance(v, (int, float)) and not isinstance(v, bool) else str(v)


def cut_modules():
    """The modules that cut parts from stock: those whose record says `cuts: true` (data, not a list in code)."""
    import glob, yaml
    out = set()
    for f in glob.glob(os.path.join(TREE, "SFO", "metadata", "modules", "*.yaml")):
        d = yaml.safe_load(open(f, encoding="utf-8")) or {}
        if d.get("cuts") is True:
            out.add(d["identity"]["key"])
    return out


_CUT = None


def _basis_flow(en):
    q = lambda s: '"' + str(s).replace('"', '\\"') + '"'
    parts = [f"of: [{', '.join(en['of'])}]"]
    for k in ("rule", "tier"):
        if en.get(k): parts.append(f"{k}: {en[k]}")
    if en.get("review") and not en.get("rule"): parts.append("review: true")
    if en.get("source"): parts.append(f"source: {q(en['source'])}")
    if en.get("note"): parts.append(f"note: {q(en['note'])}")
    return "{ " + ", ".join(parts) + " }"


def part_basis(eq, it):
    """The basis a derived part gets when its list item says none: its shares as the equipment's built_of says (a Discovery II
    rule, or shares chosen by a real device where one exists), its stock cut with the cutting loss where a cutting module makes
    it, else built in whole or bought as the good; the box by rule."""
    name = eq["identity"]["name"]; mass = (eq.get("physical") or {}).get("mass") or 0
    share = (it["mass"] / mass) if mass else 0
    bo = eq.get("built_of") or {}
    eq_rule = bo.get("shares") or next((b.get("rule") for b in eq.get("basis") or [] if "built_of" in (b.get("of") or [])), None)
    if eq_rule and eq_rule != "rule.first-design-parts" and eq_rule.startswith("rule."):
        first = {"of": ["physical.mass", "fit"], "rule": eq_rule}
    elif False:
        first = {"of": ["physical.mass", "fit"], "rule": eq_rule}
    elif eq_rule == "rule.first-design-parts":
        first = {"of": ["physical.mass", "fit"], "tier": "invented", "review": True, "note": f"Its share of the {name} ({share:.0%}), chosen."}
    else:
        first = {"of": ["physical.mass", "fit"], "rule": "rule.shares-chosen"}
    out = [first]
    if it.get("item"):
        global _CUT
        if _CUT is None:
            _CUT = cut_modules()
        if it.get("module") in _CUT and str(it["item"]).startswith("stock.") and not bo.get("whole"):
            out.append({"of": ["made_from", "making"], "rule": "rule.cut-loss"})
        else:
            out.append({"of": ["made_from", "making"], "rule": "rule.built-in-whole"})
    out.append({"of": ["physical.length", "physical.width", "physical.height"], "rule": "rule.fitted-within"})
    return out


def effective_basis(eq, it):
    """The derived defaults, each replaced by the item's own entry of the same `of` where it gives one."""
    own = {tuple(b["of"]): b for b in it.get("basis") or []}
    out = [own.pop(tuple(d["of"]), d) for d in part_basis(eq, it)]
    return out + list(own.values())


def part_yaml(eq, folder, it):
    """The YAML text of one derived part of equipment record `eq` (a dict), filed under `folder`, from list item `it`:
    {code, name, description?, mass, length, width, height, item?, quantity?, module?, count?, basis?}. The registry crate writes
    the same text (crates/core/registry/src/lib.rs, derived_parts) and parses it, so the two never drift in meaning."""
    q = lambda s: '"' + str(s).replace('"', '\\"') + '"'
    lines = ["# yaml-language-server: $schema=../../../schema/part.schema.yaml", "identity:", f"  key: part.{it['code'].lower()}", f"  code: {it['code']}", f"  name: {q(it['name'])}", "  revision: draft",
             f"  description: {q(it.get('description', ''))}", "physical:", f"  mass: {_num(it['mass'])}", f"  length: {_num(it['length'])}", f"  width: {_num(it['width'])}", f"  height: {_num(it['height'])}"]
    if it.get("item"):
        lines += ["made_from:", f"  - item: {it['item']}"] + ([f"    quantity: {_num(it['quantity'])}"] if it.get("quantity") is not None else [])
    if it.get("module"):
        lines += ["making:", f"  module: {it['module']}"]
    lines += ["fit:", f"  count: {it.get('count', 1)}", "basis:"] + [f"  - {_basis_flow(en)}" for en in effective_basis(eq, it)]
    return "\n".join(lines) + "\n"


def derived_parts(eq):
    """[(code, yaml text)] for every item of an equipment record's built_of.list."""
    bo = eq.get("built_of") or {}
    return [(it["code"], part_yaml(eq, bo.get("parts"), it)) for it in bo.get("list") or []]
