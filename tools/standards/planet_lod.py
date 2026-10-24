"""Planet Simulator's logic, once: read the root, classify a sample into a layer, give a deciding factor its strength from a
place's signature, rank landform patterns, and say a level's or a record's context. The matcher, the context lookup
and the generated module (planet_lod_module.py copies these functions into it) all use these, so the sim, the mesher,
the LOD app and the agents get one answer.

A signature (what a place is, for the matcher):
    {"fields": {"aridity": 3.2, "planet-field.ice-lgm": 800.0},   # SI, by a field's slug or key
     "classes": {"climate": 4, "zones": 9},                        # a layer's slug or key: its class id
     "landmarks": ["water-gap", "planet-landmark.delta"],         # nearby landmark types
     "factors": {"TECT_UPLIFT": 0.7}}                             # strengths given outright, 0 to 1
"""
import glob, os, math

ORDER = ("planet-level", "planet-field", "planet-layer", "planet-line", "planet-landmark", "planet-event", "planet-handler",
         "planet-measure", "planet-factor", "planet-pattern", "planet-file", "planet-model", "lod-model", "planet-feature",
         "planet-process", "planet-view", "planet-target", "planet-gate", "planet-run-profile", "planet-object")


def load(tree):
    """Every Planet Simulator record, by kind, then by key."""
    import yaml
    out = {k: {} for k in ORDER}
    for f in sorted(glob.glob(os.path.join(tree, "PlanetSim", "metadata", "**", "*.yaml"), recursive=True) + glob.glob(os.path.join(tree, "PlanetLOD", "metadata", "**", "*.yaml"), recursive=True)):      # (the meshing models are Planet LOD's)
        with open(f, encoding="utf-8") as stream:
            d = yaml.safe_load(stream) or {}
        k = (d.get("identity") or {}).get("key", "")
        if k.split(".")[0] in out:
            out[k.split(".")[0]][k] = d
    return out


def _key(kind, name):
    return name if name.startswith(kind + ".") else f"{kind}.{name}"


def _finite(value):
    """Invalid/missing readings stay unknown, including bool, infinity and NaN."""
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value) if math.isfinite(value) else None


def _value(sig, field):
    f = sig.get("fields") or {}
    for n in (field, field.split(".", 1)[-1]):
        if n in f:
            return _finite(f[n])
    return None


def _class(sig, layer):
    c = sig.get("classes") or {}
    for n in (layer, layer.split(".", 1)[-1]):
        if n in c:
            value = _finite(c[n])
            return int(value) if value is not None and value >= 0 and value.is_integer() else None
    return None


def _member(c, value, sig):
    """A class's membership of a code: True, False, or None (unknown: unresolved, possible, or an and-field unknown)."""
    if value in c.get("possible", []):
        return None
    if c.get("unresolved"):
        return None if not c["codes"] or value in c["codes"] else False
    if value not in c["codes"]:
        return False
    for cond in c.get("and", []):
        v = _value(sig, cond["field"])
        if v is None or not float(v).is_integer() or v in cond.get("possible", []):
            return None
        if v not in cond["codes"]:
            return False
    return True


def holds(cond, sig):
    """Inclusive SI min/max; legacy op/value remains for layer classification only."""
    v = _value(sig, cond["field"])
    if v is None:
        return None
    if "op" in cond:
        return {"<": v < cond["value"], "<=": v <= cond["value"], ">": v > cond["value"], ">=": v >= cond["value"], "==": v == cond["value"]}[cond["op"]]
    if not any(k in cond for k in ("min", "max")):
        return None
    return ("min" not in cond or v >= cond["min"]) and ("max" not in cond or v <= cond["max"])


def classify(R, layer, sig):
    """A sample's class id in a layer: with default and order, the default replaced by each rule in order that holds;
    else the first class whose conditions all hold. None when nothing decides."""
    L = R["planet-layer"][_key("planet-layer", layer)]
    classes = {c["id"]: c for c in L.get("classes") or []}
    ok = lambda c: c.get("when") and all(holds(x, sig) for x in c["when"])
    if L.get("default"):
        cid = L["default"]
        for i in L.get("order") or []:
            if ok(classes[i]):
                cid = i
        return cid
    return next((i for i, c in sorted(classes.items()) if ok(c)), None)


def strength(R, factor, sig, missing=0.0, signal=None):
    """A deciding factor's strength (0 to 1) at a place: given outright, else by its signal (a field ramped, a class
    among some, a landmark nearby), else missing (0 by default)."""
    d = R["planet-factor"][factor]
    if signal is None and d.get("reading", {}).get("status") == "unavailable":
        return missing
    given = sig.get("factors") or {}
    if signal is None and d.get("source") in given:
        value = _finite(given[d["source"]])
        return max(0.0, min(1.0, value)) if value is not None else missing
    s = signal if signal is not None else (d.get("signal") or {})
    # The offline signature has no spatial feature census: do not reuse a global or landmark score.
    # Spatial evidence is evaluated by planet-forge against exact checkpoint geometry/provenance.
    if s.get("features"):
        return missing
    if s.get("field"):
        v = _value(sig, s["field"])
        if v is not None and s["to"] != s["from"]:
            return max(0.0, min(1.0, (v - s["from"]) / (s["to"] - s["from"])))
    if s.get("layer"):
        c = _class(sig, s["layer"])
        if c is not None:
            return 1.0 if c in s.get("classes", []) else 0.0
    if s.get("landmarks") and "landmarks" in sig:
        near = {_key("planet-landmark", m) for m in sig.get("landmarks") or []}
        return 1.0 if near & set(s["landmarks"]) else 0.0
    return missing


def preference(factor, value):
    """Match a known normalized strength to explicit high/low/soft-band intent."""
    if value is None:
        return 0.0
    want = factor["want"]
    if want == "high":
        return value
    if want == "low":
        return 1.0 - value
    if want == "band":
        lo, hi = factor["min"], factor["max"]
        width = factor["soft"] * (hi - lo)
        return max(0.0, min(1.0, (value - lo + width) / width, (hi + width - value) / width))
    raise ValueError("unknown factor preference: " + str(want))


def entry_score(R, entry, sig):
    """None is unknown, distinct from a known zero. TIME uses actual per-form age."""
    if entry.get("unresolved"):
        return None
    conditions = [holds(c, sig) for c in entry.get("when", [])]
    if False in conditions:
        return 0.0
    if None in conditions:
        return None
    factor = R["planet-factor"][entry["factor"]]
    if entry["factor"] == "planet-factor.time":
        age = _finite(sig.get("form_age_s"))
        tau = _finite(entry.get("tau_relax_s"))
        return math.exp(-age / tau) if age is not None and age >= 0 and tau is not None and tau > 0 else None
    if "signal" not in entry and factor.get("reading", {}).get("status") == "unavailable":
        return None
    signal = entry.get("signal", factor.get("signal", {}))
    want = entry["want"]
    if want in ("in", "not-in"):
        if factor.get("kind") != "categorical":
            return None
        if signal.get("field"):
            value = _value(sig, signal["field"])
        elif signal.get("layer"):
            value = _class(sig, signal["layer"])
        else:
            return None
        mapping = {c["key"]: c for c in factor.get("class_map", [])}
        selected = entry.get("classes", [])
        known = {x for c in mapping.values() for x in c["codes"] + c.get("possible", [])}
        if value is None or value not in known or not selected or any(k not in mapping for k in selected):
            return None
        member = [_member(mapping[k], value, sig) for k in selected]
        if True in member:
            inside = True
        elif None in member:
            return None
        else:
            inside = False
        return float(inside if want == "in" else not inside)

    if factor.get("kind") == "categorical":
        return None
    if want == "band":
        value = _value(sig, signal["field"]) if signal.get("field") else None
    else:
        value = strength(R, entry["factor"], sig, missing=None, signal=entry.get("signal"))
    return preference(entry, value) if value is not None else None


def match(R, sig, top=10, family=None):
    """Rank only evidence-backed scores. Required unknowns are visible with score zero.

    Recognition trigger rules and gate-only patterns never become placements from scores.
    Optional missing terms retain their weight (score zero), without satisfying low wants.
    """
    out = []
    for k, p in R["planet-pattern"].items():
        if family and p["family"] != family:
            continue
        conditions = [holds(c, sig) for c in p.get("when", [])]
        if False in conditions:
            continue
        state = "unknown" if None in conditions else "pass"
        total = score = 0.0
        fired = []
        for entry in p["factors"]:
            value = entry_score(R, entry, sig)
            if entry.get("required"):
                if value == 0:
                    state = "fail"
                    break
                if value is None:
                    state = "unknown"
            else:
                weight = entry["weight"]
                total += weight
                score += weight * (value if value is not None else 0)
            if value is not None and value > 0:
                fired.append((R["planet-factor"][entry["factor"]]["source"], entry["weight"], round(value, 3)))
        if state == "fail":
            continue
        if p.get("trigger_rule"):
            state = "unknown"
        score = score / total if total and state == "pass" else 0.0
        out.append({"key": k, "name": p["identity"]["name"], "family": p["family"],
                    "state": state, "score": round(score, 4),
                    "fired": sorted(fired, key=lambda x: -x[1] * x[2])})
    return sorted(out, key=lambda x: (-x["score"], x["key"]))[:top]


LINKS = ("levels", "parent", "children", "bounded_by", "made_by", "handled_by", "measured_by", "class_of", "derived_from", "vocabulary", "is")


def _refs(d):
    """Every key a record names: its context links, its fields' and conditions' and factors' references."""
    out = []
    def walk(x):
        if isinstance(x, dict):
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)
        elif isinstance(x, str) and x.startswith(("planet-", "ground-vocabulary.")) and "." in x and " " not in x:
            out.append(x)
    walk({k: v for k, v in d.items() if k not in ("identity", "basis")})
    return sorted(set(out))


def context(R, target):
    """What a tool working at a level, or on a record, should take: for a level, the records meaningful there by kind,
    the anchors its parent hands down frozen, its parent and child levels; for a record, what it names and what names it."""
    every = {k: d for kind in R.values() for k, d in kind.items()}
    lv = R["planet-level"]
    if _key("planet-level", target) in lv:
        key = _key("planet-level", target)
        names = list(lv)
        order = sorted(names, key=lambda k: -lv[k]["span"])
        i = order.index(key)
        here = {}
        for k, d in every.items():
            ladder = d.get("ladder") if k.startswith("planet-feature.") else None
            meaningful = key in (d.get("levels") or [])
            if ladder and len(ladder) == 2 and all(x in order for x in ladder):
                meaningful = order.index(ladder[0]) <= i <= order.index(ladder[1])
            if meaningful and not k.startswith("planet-level."):
                here.setdefault(k.split(".")[0], []).append(k)
        return {"level": key, "span_m": lv[key]["span"], "spacing_m": lv[key]["spacing"], "parent": order[i - 1] if i else None,
                "child": order[i + 1] if i + 1 < len(order) else None, "exists": lv[key].get("exists"), "from_parent": lv[key].get("from_parent"),
                "records": {k: sorted(v) for k, v in sorted(here.items())}}
    key = target if target in every else next((k for k in every if k.split(".", 1)[-1] == target or k.endswith("." + target)), None)
    if not key:
        return {"error": f"no level or record {target}"}
    d = every[key]
    return {"record": key, "name": d["identity"]["name"], "about": d["identity"].get("about"),
            "names": _refs(d), "named_by": sorted(k for k, x in every.items() if k != key and key in _refs(x)),
            "source": d.get("source")}
