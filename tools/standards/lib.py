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
