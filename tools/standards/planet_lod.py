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
import glob, os

ORDER = ("planet-level", "planet-field", "planet-layer", "planet-line", "planet-landmark", "planet-event", "planet-handler",
         "planet-measure", "planet-factor", "planet-pattern", "planet-file", "planet-model", "lod-model")


def load(tree):
    """Every Planet Simulator record, by kind, then by key."""
    import yaml
    out = {k: {} for k in ORDER}
    for f in sorted(glob.glob(os.path.join(tree, "PlanetSim", "metadata", "**", "*.yaml"), recursive=True) + glob.glob(os.path.join(tree, "PlanetLOD", "metadata", "**", "*.yaml"), recursive=True)):      # (the meshing models are Planet LOD's)
        d = yaml.safe_load(open(f, encoding="utf-8")) or {}
        k = (d.get("identity") or {}).get("key", "")
        if k.split(".")[0] in out:
            out[k.split(".")[0]][k] = d
    return out


def _key(kind, name):
    return name if name.startswith(kind + ".") else f"{kind}.{name}"


def _value(sig, field):
    f = sig.get("fields") or {}
    for n in (field, field.split(".", 1)[-1]):
        if n in f and f[n] is not None:
            return float(f[n])
    return None


def _class(sig, layer):
    c = sig.get("classes") or {}
    for n in (layer, layer.split(".", 1)[-1]):
        if n in c:
            return c[n]
    return None


def holds(cond, sig):
    """One condition ({field, op, value}) over a signature; None when the field is not in it."""
    v = _value(sig, cond["field"])
    if v is None:
        return None
    return {"<": v < cond["value"], "<=": v <= cond["value"], ">": v > cond["value"], ">=": v >= cond["value"], "==": v == cond["value"]}[cond["op"]]


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


def strength(R, factor, sig):
    """A deciding factor's strength (0 to 1) at a place: given outright, else by its signal (a field ramped, a class
    among some, a landmark nearby), else 0."""
    d = R["planet-factor"][factor]
    given = sig.get("factors") or {}
    if d.get("source") in given:
        return max(0.0, min(1.0, float(given[d["source"]])))
    s = d.get("signal") or {}
    if s.get("field"):
        v = _value(sig, s["field"])
        if v is not None and s["to"] != s["from"]:
            return max(0.0, min(1.0, (v - s["from"]) / (s["to"] - s["from"])))
    if s.get("layer"):
        c = _class(sig, s["layer"])
        if c is not None:
            return 1.0 if c in s.get("classes", []) else 0.0
    if s.get("landmarks"):
        near = {_key("planet-landmark", m) for m in sig.get("landmarks") or []}
        return 1.0 if near & set(s["landmarks"]) else 0.0
    return 0.0


def match(R, sig, top=10, family=None):
    """Landform patterns ranked for a signature: each one's factors' strengths weighted (its hard conditions first: one
    that fails, it does not match). Returns [{key, family, score, fired: [(factor, weight, strength)], failed}]."""
    out = []
    for k, p in R["planet-pattern"].items():
        if family and p["family"] != family:
            continue
        failed = [c for c in p.get("when") or [] if holds(c, sig) is False]
        if failed:
            continue
        tw = sum(f["weight"] for f in p["factors"]) or 1.0
        fired = [(R["planet-factor"][f["factor"]]["source"], f["weight"], round(strength(R, f["factor"], sig), 3)) for f in p["factors"]]
        score = sum(w * s for _, w, s in fired) / tw
        out.append({"key": k, "name": p["identity"]["name"], "family": p["family"], "score": round(score, 4),
                    "fired": sorted([x for x in fired if x[2] > 0], key=lambda x: -x[1] * x[2])})
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
            if key in (d.get("levels") or []) and not k.startswith("planet-level."):
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
