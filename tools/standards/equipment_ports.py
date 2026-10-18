"""Electrical ports on a ship's equipment (docs/proposals/equipment-ports.md, agreed by ships 2026-10-18): each powered
record gets its power ports on the bus its draw puts it on, a pair of data ports, and `internal` saying how they are
joined inside it, so that players' laid wires snap to them by the contract's rules. The generated converters and
distribution boards (converters.py) take their ports from here too.

Buses (ships' studio, docs/ship-wiring.md in the game): 28 V DC up to 1 kW, 270 V DC up to 30 kW, 3 kV DC above and
for every source. Places: from the installed model's empties where it has them (power_data, power_entry_*, the
converters' input_*/output_*/data/coolant_*), else on its -x end face, marked invented.

    python3 tools/standards/equipment_ports.py           # write the ports into the hand-written records (idempotent)
    python3 tools/standards/equipment_ports.py --check   # exit 1 if any record differs from what it would write
"""
import glob
import json
import math
import os
import struct
import sys
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata/equipment")
ASSETS = os.environ.get("UNIVERSE_ASSETS", os.path.expanduser("~/git/freefall-assets"))
MARK = "# ports and internal: written by tools/standards/equipment_ports.py, change them there.\n"

BUSES = [(1000.0, 28.0, "dc-28v"), (30000.0, 270.0, "dc-270v"), (math.inf, 3000.0, "dc-3kv")]     # (W at most, V, channel): ships' limits
HV = 3000.0
POLES = [150, 400, 650, 1000]          # A: the HV DC pole connectors (kit_catalogue.py)
SHELLS = [(15, 17), (55, 25), (120, 33)]   # (A, shell) for a low-bus way
PAIR_MAX = 500.0                       # A a pole pair carries where a load needs more than one pair: an air-bundled 400 mm2 aluminium feeder's (invented)
DATA = "can-a"                         # the ship's one control bus (invented: a CAN-class two-wire bus, daisy-chained)
FACE = {11: 0.022, 17: 0.03, 25: 0.04, 33: 0.052, 150: 0.04, 400: 0.055, 650: 0.07, 1000: 0.085}    # m across a mated connector
GAP = 0.015                            # m between neighbouring receptacles (invented)

SOURCES = {"battery": "both", "power_plant": "out"}


def r(x, nd=3):
    return round(float(x), nd) + 0.0      # (no -0.0)


# ------------------------------------------------------------------ the model's empties
def _quat_rot(q, v):
    x, y, z, w = q
    tx, ty, tz = 2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])
    return [v[0] + w * tx + (y * tz - z * ty), v[1] + w * ty + (z * tx - x * tz), v[2] + w * tz + (x * ty - y * tx)]


def _quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]


def empties(rec):
    """{name: (at, facing)} of the installed model's empty nodes, in the record's frame (x along, y across, z up from
    its base; the glTF export is +Y up, so record (x, y, z) = glTF (x, -z, y)); facing is the empty's +Y arrow."""
    v = (rec.get("visual") or {}).get("path")
    p = v and os.path.join(ASSETS, v, "model.glb")
    if not p or not os.path.exists(p):
        return {}
    b = open(p, "rb").read()
    n = struct.unpack("<I", b[12:16])[0]
    nodes = json.loads(b[20:20 + n])["nodes"]
    out = {}

    def walk(i, t, q, s):
        nd = nodes[i]
        lt, lq, ls = nd.get("translation", [0, 0, 0]), nd.get("rotation", [0, 0, 0, 1]), nd.get("scale", [1, 1, 1])
        rt = _quat_rot(q, [lt[k] * s[k] for k in range(3)])
        t2, q2, s2 = [t[k] + rt[k] for k in range(3)], _quat_mul(q, lq), [s[k] * ls[k] for k in range(3)]
        if "mesh" not in nd and not nd.get("children"):
            f = _quat_rot(q2, [0, 1, 0])
            out[nd.get("name", str(i))] = ([r(t2[0]), r(-t2[2]), r(t2[1])], [r(f[0], 4), r(-f[2], 4), r(f[1], 4)])
        for c in nd.get("children", []):
            walk(c, t2, q2, s2)
    kids = {c for nd in nodes for c in nd.get("children", [])}
    for i in range(len(nodes)):
        if i not in kids:
            walk(i, [0, 0, 0], [0, 0, 0, 1], [1, 1, 1])
    return out


# ------------------------------------------------------------------ connectors and their pins
def pole(amps):
    return next(a for a in POLES if a >= amps)


def shell(amps):
    return next((sh for a, sh in SHELLS if a >= amps), None)


def pins_shell(sh):
    if sh in (11, 17):
        return [{"contact": "1", "role": "plus"}, {"contact": "2", "role": "minus"}, {"contact": "3", "role": "earth"}] + [{"contact": str(c), "role": "spare"} for c in range(4, 8)]
    if sh == 25:
        return [{"contact": "A", "role": "plus"}, {"contact": "B", "role": "minus"}, {"contact": "C", "role": "earth"}, {"contact": "D", "role": "spare"}]
    return [{"contact": "A", "role": "plus"}, {"contact": "B", "role": "minus"}]


def way(pid, flow, half, volts, channel, amps, power, note=None, sh=None):
    """One low-bus way: a circular shell carrying plus, minus (and earth where it has a contact for it)."""
    sh = sh or shell(amps)
    p = {"id": pid, "flow": flow, "medium": "power", "interface": f"interface.circular-shell-{sh}", "half": half, "make": "manual",
         "connector": f"connector.circular-shell-{sh}", "voltage": volts, "current": r(amps, 2), "power": r(power, 1), "channel": channel, "pins": pins_shell(sh)}
    return p, FACE[sh]


def pole_pair(base, flow, half, volts, channel, amps, power, size=None):
    """A DC feed by two single-pole connectors keyed plus and minus."""
    a = size or pole(amps)
    out = []
    for pol, sign in (("plus", "+"), ("minus", "-")):
        out.append(({"id": f"{base}{sign}", "flow": flow, "medium": "power", "interface": f"interface.hvdc-pole-{a}a", "half": half, "make": "manual",
                     "connector": f"connector.hvdc-pole-{a}a", "voltage": volts, "current": r(amps, 2), "power": r(power, 1), "channel": channel, "key": pol,
                     "pins": [{"contact": "1", "role": pol}]}, FACE[a]))
    return out


def data_ports(n=2):
    return [({"id": f"data-{i}", "flow": "both", "medium": "signal", "interface": "interface.circular-shell-11", "half": "female", "make": "manual",
              "connector": "connector.circular-shell-11", "channel": DATA,
              "pins": [{"contact": "1", "role": "signal-high"}, {"contact": "2", "role": "signal-low"}, {"contact": "3", "role": "return"}, {"contact": "4", "role": "screen"}]
              + [{"contact": str(c), "role": "spare"} for c in range(5, 8)]}, FACE[11]) for i in range(1, n + 1)]


def feed(base, flow, half, power):
    """The power ports for a draw (or a source's output) of `power` W: its bus, its connector, as many pole pairs as
    the current needs on the 3 kV bus. Returns ([(port, face)], channel)."""
    limit, volts, ch = next(b for b in BUSES if power <= b[0]) if flow == "in" else BUSES[-1]
    amps = power / volts
    if volts < HV:
        sh = shell(amps)
        if sh:
            return [way(f"{base}", flow, half, volts, ch, amps, power)], ch
        return pole_pair(base, flow, half, volts, ch, amps, power), ch
    n = 1 if amps <= POLES[2] else math.ceil(amps / PAIR_MAX)
    out = []
    for i in range(n):
        out += pole_pair(f"{base}{i + 1 if n > 1 else ''}", flow, half, volts, ch, amps / n, power / n)
    return out, ch


# ------------------------------------------------------------------ places
def _perp(f):
    """A direction across a face facing f to lay a row along: horizontal where it can be."""
    if abs(f[2]) > 0.9:
        return [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]
    h = [-f[1], f[0], 0.0]
    n = math.hypot(h[0], h[1])
    return [h[0] / n, h[1] / n, 0.0], [0.0, 0.0, 1.0]


def lay(items, centre, facing, width, note):
    """Lay receptacles in rows across a face: centred on `centre`, rows `width` long at most, stacked upward."""
    along, up = _perp(facing)
    rows, row, used = [], [], 0.0
    for p, w in items:
        if row and used + w > width:
            rows.append(row)
            row, used = [], 0.0
        row.append((p, w))
        used += w + GAP
    rows.append(row)
    pitch = max(w for _p, w in items) + GAP
    out = []
    for j, row in enumerate(rows):
        span = sum(w for _p, w in row) + GAP * (len(row) - 1)
        x = -span / 2
        dz = (j - (len(rows) - 1) / 2) * pitch
        for p, w in row:
            c = x + w / 2
            x += w + GAP
            p = dict(p)
            p["at"] = [r(centre[k] + along[k] * c + up[k] * dz) for k in range(3)]
            p["facing"] = [r(v, 4) for v in facing]
            p["note"] = note
            out.append(p)
    return out


def end_face(rec):
    ph = rec["physical"]
    return [r(-ph["length"] / 2), 0.0, r(min(ph["height"] * 0.5, max(0.06, ph["height"] * 0.3)))], [-1.0, 0.0, 0.0], ph["width"] * 0.9


INVENTED = "place invented (the model has no empty for it): on its -x end face, for review"


def place(rec, items, at=None, name=None):
    if at:
        c, f = at
        return lay(items, c, f, max(0.3, rec["physical"]["width"] * 0.9), f"place from the model's empty {name}")
    c, f, w = end_face(rec)
    return lay(items, c, f, w, INVENTED)


# ------------------------------------------------------------------ what each kind gets
def build(rec):
    """(ports, internal) for a hand-written record, or None when it takes no electrical ports here."""
    fn = rec.get("function") or {}
    kind = fn.get("kind")
    em = empties(rec)
    if kind == "switchgear":
        return switchgear(rec, em)
    if kind in SOURCES:
        power = fn.get("rate") or fn.get("output")
        items, ch = feed("dc-out", SOURCES[kind], "female", power)
        for p, _w in items:
            if SOURCES[kind] == "both":
                p["id"] = p["id"].replace("dc-out", "dc")
        what = "charged and discharged through them" if SOURCES[kind] == "both" else "its output"
    else:
        power = (rec.get("needs") or {}).get("power")
        if not power:
            return None
        items, ch = feed("dc-in", "in", "male", power)
        what = "its draw"
    data = data_ports()
    pd = em.get("power_data")
    ports = place(rec, items + data, pd, "power_data")
    pids = [p["id"] for p, _w in items]
    internal = [{"kind": "joined", "ports": ["data-1", "data-2"], "note": "the control bus passes through, so units daisy-chain on it"}]
    if len(pids) > 2:
        internal.insert(0, {"kind": "joined", "ports": pids, "note": f"its {len(pids) // 2} pole pairs in parallel: {what} shares them"})
    return ports, internal


WAYS_PER_ENTRY = 8


def switchgear(rec, em):
    """The 3 kV bus: four entries of eight ways, each way a pole pair of 1000 A, any of them a source's or a load's;
    every way through its own breaker onto one bus bar."""
    carries = rec["function"]["carries"]
    entries = sorted(k for k in em if k.startswith("power_entry_")) or [None] * 4
    ports, ids = [], []
    for e, name in enumerate(entries):
        items = []
        for w in range(WAYS_PER_ENTRY):
            n = e * WAYS_PER_ENTRY + w + 1
            items += pole_pair(f"way-{n}", "both", "female", HV, "dc-3kv", 1000, 1000 * HV, size=1000)
        ids += [p["id"] for p, _w in items]
        ports += place(rec, items, em.get(name), name) if name else place(rec, items)
    for p in ports:
        p["note"] += "; ways: a source's or a load's"
    ports += place(rec, data_ports(), None)
    return ports, [{"kind": "switched", "ports": ids, "note": f"one bus bar of {carries / 1e6:g} MW; each way through its own breaker"},
                   {"kind": "joined", "ports": ["data-1", "data-2"], "note": "the control bus passes through"}]


# ------------------------------------------------------------------ writing
def block(ports, internal):
    lines = [MARK, "ports:\n"]
    for p in ports:
        lines.append("- " + yaml.safe_dump(p, default_flow_style=True, sort_keys=False, width=100000, allow_unicode=True).strip() + "\n")
    lines.append("internal:\n")
    for i in internal:
        lines.append("- " + yaml.safe_dump(i, default_flow_style=True, sort_keys=False, width=100000, allow_unicode=True).strip() + "\n")
    return "".join(lines)


def strip(text):
    """The record's text without the block this tool wrote (always its last)."""
    i = text.find(MARK)
    return text if i < 0 else text[:i]


def faults():
    """Every equipment port against the contract: its interface and connector exist and agree, its key is one of the
    interface's keys, its pins name the interface's contacts, and its record's internal names its own ports."""
    load = lambda d: {yaml.safe_load(open(f))["identity"]["key"]: yaml.safe_load(open(f)) for f in glob.glob(os.path.join(ROOT, "standards/SFO/metadata", d, "*.yaml"))}
    ifs, cons = load("interfaces"), load("connectors")
    bad = []
    for f in sorted(glob.glob(os.path.join(M, "*.yaml"))):
        rec = yaml.safe_load(open(f))
        k = rec["identity"]["key"]
        ids = [p["id"] for p in rec.get("ports") or []]
        if len(set(ids)) != len(ids):
            bad.append(f"{k}: port ids repeat")
        for p in rec.get("ports") or []:
            i = ifs.get(p["interface"])
            if not i:
                bad.append(f"{k} {p['id']}: no {p['interface']}")
                continue
            if p.get("connector") and (cons.get(p["connector"]) or {}).get("interface") != p["interface"]:
                bad.append(f"{k} {p['id']}: {p['connector']} is not built to {p['interface']}")
            if ("key" in p) != bool(i.get("keys")) or p.get("key", None) not in (i.get("keys") or [None]):
                bad.append(f"{k} {p['id']}: key {p.get('key')!r}, its interface's keys {i.get('keys')}")
            cs = {c["id"] for c in i.get("contacts") or []}
            for pin in p.get("pins") or []:
                if pin["contact"] not in cs:
                    bad.append(f"{k} {p['id']}: contact {pin['contact']} not in {p['interface']}")
            if p.get("pins") and len({q["contact"] for q in p["pins"]}) != len(p["pins"]):
                bad.append(f"{k} {p['id']}: a contact twice")
            if p.get("medium") in ("power", "signal") and cs and not p.get("pins"):
                bad.append(f"{k} {p['id']}: no pins")
        for j in rec.get("internal") or []:
            for q in (j.get("ports") or []) + (j.get("from") or []) + (j.get("to") or []):
                if q not in ids:
                    bad.append(f"{k} internal: no port {q}")
    return bad


def main(check=False):
    usage = yaml.safe_load(open(os.path.join(ROOT, "docs/asset-usage.yaml")))["equipment"]
    wrote, differ, done = 0, [], 0
    for f in sorted(glob.glob(os.path.join(M, "*.yaml"))):
        text = open(f, encoding="utf-8").read()
        if "Written by tools/" in text[:400]:
            continue
        rec = yaml.safe_load(text)
        if rec["identity"]["key"] not in usage:
            continue
        base = strip(text)
        if "\nports:" in base or base.startswith("ports:"):
            print(f"  {rec['identity']['key']}: has hand-written ports; left alone")
            continue
        b = build(yaml.safe_load(base))
        new = base if b is None else base.rstrip("\n") + "\n" + block(*b)
        done += b is not None
        if new != text:
            differ.append(rec["identity"]["key"])
            if not check:
                open(f, "w", encoding="utf-8").write(new)
                wrote += 1
    if check:
        bad = faults() + [f"{k}: differs from what tools/standards/equipment_ports.py writes" for k in differ]
        for b in bad[:20]:
            print("  " + b)
        print(f"equipment ports: {done} records written here; {len(bad)} fault(s)")
    else:
        print(f"equipment ports: {done} records with ports; {wrote} written")


if __name__ == "__main__":
    main("--check" in sys.argv)
