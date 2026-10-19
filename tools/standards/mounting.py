"""Physical mounts (docs/proposals/physical-mounts.md, agreed with ships 2026-10-18): how equipment is held on a ship's
frame. Writes the load interfaces (threaded inserts, the kit clamp, the deck track), the strut clamps that join a
holder to a kit strut, every SFO 19 mount's sockets and holder, a cradle (adapter) for each mount too heavy or too big
for a deck, the deck panels, their templates and their KIT- stock.

    python3 tools/standards/mounting.py           # write them (idempotent)
    python3 tools/standards/mounting.py --check   # faults: a mount with no sockets, a socket no bolt carries, a cradle short of its mount

The rules, chosen (every figure here is review-pending until ships and the owner approve it or a source replaces it):
- Bolts: ISO metric, property class 10.9 (ISO 898-1: proof 830 MPa, tensile 1,040 MPa), the frames' bolt sizes. A
  bolt holds its proof load in tension and 0.6 of its tensile strength over its stress area in shear. A socket takes
  the smallest bolt holding its point's tension and shear times the ultimate factor (1.5).
- Stiffness of a bolted joint: the bolt's shank, E A / L over a grip of 4 diameters, along it; half that across.
- A strut clamp: a 6061 split collar as long as the strut is wide, a fifth of its width thick, closed by two bolts
  (the smallest at least an eighth of the strut's width) at 0.7 of proof load. It holds, pulled off or pressed on,
  the two bolts' preload; slipping along or round the strut, friction 0.3 at both halves.
- The deck track: aircraft seat track (studs on a 25.4 mm pitch), rails 0.25 m apart; a stud holds 9 kN pulled and
  13 kN across (from memory of seat-track practice). A mount goes on a deck when its footprint fits a 2 x 1 m panel and
  its points are within a stud times the factor; any other gets a cradle.
- A cradle: two kit tubes along x over the frame's struts, fixings clamped at the middle of every 1 m of them (so it
  lands on a 1 m or a 0.5 m lattice), cross tubes under the sockets and at the ends; every tube the lightest 6061 (else
  4340) on the kit ladder whose bending stress under the mount's points times the factor is within its yield.
"""
import glob, math, os, sys
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
M = os.path.join(ROOT, "standards/SFO/metadata")
HEAD = "# Written by tools/standards/mounting.py: change it there.\n"
KEEP = ("visual", "model_requests")
sys.path.insert(0, HERE)
from packages import FORMS as PACKAGE_OF  # noqa: E402

FACTOR = 1.5                # ultimate factor (invented, review)
PROOF, TENSILE, E_STEEL = 830e6, 1040e6, 200e9
AS = {10: 58.0, 16: 157.0, 20: 245.0, 30: 561.0, 42: 1120.0, 56: 2030.0, 64: 2680.0, 80: 4340.0, 100: 6990.0}   # mm2 tensile stress area (ISO 898-1), the sizes the frames stock
PRELOAD, MU = 0.7, 0.3
KIT_D = [0.06, 0.08, 0.1, 0.12, 0.15, 0.16, 0.2, 0.25]
TRACK_PITCH, RAIL_GAP = 0.0254, 0.25
STUD = {"tension": 9e3, "shear": 13e3, "compression": 18e3}
TRACK_K = {"axial": 5e7, "shear": 2.5e7}
DECKS = [(0.5, 0.5), (1.0, 0.5), (1.0, 1.0), (2.0, 1.0)]
AL, ST = "material.aluminium-alloy-6061", "material.high-strength-steel-4340"
RHO = {AL: 2700.0, ST: 7850.0}
YIELD = {AL: 270e6, ST: 1300e6}
DRY = "--check" in sys.argv        # (check only: count what is out of date, write nothing)
REVIEW = "Review-pending (docs/proposals/physical-mounts.md): chosen, not approved."


def r(x, n=4):
    return round(x + 0.0, n)


def load(folder):
    out = {}
    for f in sorted(glob.glob(os.path.join(M, folder, "*.yaml"))):
        d = yaml.safe_load(open(f, encoding="utf-8")) or {}
        out[os.path.basename(f)[:-5]] = (f, d)
    return out


def write(folder, slug, d, schema):
    path = os.path.join(M, folder, slug + ".yaml")
    if os.path.exists(path):
        old = yaml.safe_load(open(path, encoding="utf-8")) or {}
        d = dict(d, **{k: old[k] for k in KEEP if k in old and k not in d})
    os.makedirs(os.path.dirname(path), exist_ok=True)
    text = f"# yaml-language-server: $schema=../../schema/{schema}.schema.yaml\n" + HEAD + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=230)
    if not os.path.exists(path) or open(path, encoding="utf-8").read() != text:
        if not DRY:
            open(path, "w", encoding="utf-8").write(text)
        return 1
    return 0


# ------------------------------------------------------------------ bolts and joints
def bolt_caps(d):
    a = AS[d] * 1e-6
    return PROOF * a, 0.6 * TENSILE * a

def bolt_k(d):
    m = d / 1000
    ax = E_STEEL * math.pi * m * m / 4 / (4 * m)
    return {"axial": r(ax, -3), "shear": r(ax / 2, -3)}

def bolt_for(tension, shear):
    return next((d for d in AS if bolt_caps(d)[0] >= FACTOR * tension and bolt_caps(d)[1] >= FACTOR * shear), None)

def bolt_mass(d):
    f = os.path.join(M, "mill-stock", f"ST4340-BOLT-M{d}.yaml")
    return yaml.safe_load(open(f, encoding="utf-8"))["physical"]["mass"]


def insert_iface(d):
    t, s = bolt_caps(d)
    return {"identity": {"key": f"interface.insert-m{d}", "name": f"Threaded insert M{d}",
                         "description": f"A holder's socket: an M{d} threaded insert set in a pad, its face square to the bolt. Equipment's foot (male) is a flange with a clearance hole; the joining product is an M{d} bolt."},
            "domain": "load", "kind": "insert", "size": d / 1000, "envelope": {"tension": r(t, 0), "compression": r(2 * t, 0), "shear": r(s, 0)},
            "stiffness": bolt_k(d), "media": ["load"], "sealing": "open", "halves": "male-female", "make": "bolted",
            "face": {"length": r(2.5 * d / 1000), "width": r(2.5 * d / 1000), "height": r(d / 1000)},
            "basis": [{"of": ["envelope"], "tier": "derived", "review": True, "note": f"An ISO 898-1 class 10.9 bolt: proof 830 MPa and 0.6 of 1,040 MPa over its {AS[d]:g} mm2 stress area; pressed together, twice its proof (the pad bears). {REVIEW}"},
                      {"of": ["stiffness"], "tier": "derived", "review": True, "note": f"The shank, E A / L over a grip of 4 diameters; across, half. {REVIEW}"}]}


def clamp(D):
    b = next(d for d in AS if d >= D * 1000 / 8)
    t, L = 0.2 * D, D
    pre = PRELOAD * bolt_caps(b)[0]
    ring = math.pi * (D + t) * t * L * RHO[AL]
    lugs = 2 * 2 * (3 * b / 1000) * (2 * b / 1000) * L * RHO[AL]
    holder = 0.1 * 0.1 * 0.01 * RHO[AL]                        # the holder's lug plate the collar bolts to
    bolts = 2 * bolt_mass(b)
    mm = round(D * 1000)
    k = bolt_k(b)
    rec = {"identity": {"key": f"connector.strut-clamp-d{mm}", "name": f"Strut clamp, {mm} mm",
                        "description": f"A split collar clamped round a {mm} mm kit strut by two M{b} bolts, its lug bolted to a holder's fixing (a cradle's or a deck panel's): how a holder is fixed to a ship's frame anywhere along a strut's clear length, never at a node."},
           "kind": "strut-clamp", "size": D,
           "rating": {"tension": r(2 * pre, 0), "compression": r(2 * pre, 0), "shear": r(MU * 2 * 2 * pre, 0)},
           "stiffness": {"axial": r(2 * k["axial"], -3), "shear": r(2 * k["shear"], -3)},
           "physical": {"length": r(D + 2 * t + 6 * b / 1000), "width": r(D + 2 * t), "height": r(L), "mass": r(ring + lugs + holder + bolts, 3)},
           "built_of": [{"what": "Collar halves and lugs", "item": AL, "mass": r(ring + lugs + holder, 3), "note": f"6061, {round(t * 1000)} mm thick, {mm} mm long"},
                        {"what": "Bolts", "item": ST, "mass": r(bolts, 3), "note": f"two M{b} with nuts"}],
           "interface": "interface.kit-clamp", "provides": "both", "template": "template.strut-clamp",
           "basis": [{"of": ["physical", "rating", "stiffness"], "tier": "invented", "review": True,
                      "note": f"Sized by the rule in tools/standards/mounting.py: collar {mm} mm long and {round(t * 1000)} mm thick; two M{b} at 0.7 of proof ({pre / 1e3:.1f} kN each); pulled or pressed, the two preloads; slipping, friction 0.3 at both halves. {REVIEW}"}]}
    return rec, b


# ------------------------------------------------------------------ mounts: sockets and holder
def rect_points(n, a, b):
    corners = [(-a, -b), (a, -b), (a, b), (-a, b)]
    if n <= 4:
        return corners[:n]
    mids = [(0.0, -b), (0.0, b)] if a >= b else [(-a, 0.0), (a, 0.0)]
    if n == 6:
        return corners + mids
    if n == 8:
        return corners + [(0.0, -b), (a, 0.0), (0.0, b), (-a, 0.0)]
    per, pts = 4 * (a + b), []                 # (any other count: evenly round the rectangle from a corner)
    for i in range(n):
        s = per * i / n
        for (x0, y0), (x1, y1) in zip(corners, corners[1:] + corners[:1]):
            seg = math.dist((x0, y0), (x1, y1))
            if s <= seg + 1e-9:
                f = s / seg
                pts.append((x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)); break
            s -= seg
    return pts


def deckable(m):
    e, each = m["envelope"], m["attachment"]["each"]
    L, W = sorted((e["length"], e["width"]), reverse=True)
    return L <= 2.0 and W <= 1.0 and all(each[k] * FACTOR <= STUD[k] for k in ("tension", "shear", "compression"))


def sockets(m):
    e, att = m["envelope"], m["attachment"]
    n, pat = att["points"], att["pattern"]
    L, W = e["length"], e["width"]
    if deckable(m):
        rails = [RAIL_GAP / 2 + RAIL_GAP * k for k in range(8)]
        yb = max([y for y in rails if y <= 0.45 * W] or [RAIL_GAP / 2])
        xb = max(TRACK_PITCH, math.floor(0.45 * L / TRACK_PITCH) * TRACK_PITCH)
        per = max(2, math.ceil(n / 2))
        xs = [-xb + 2 * xb * i / (per - 1) for i in range(per)]
        xs = [round(x / TRACK_PITCH) * TRACK_PITCH for x in xs]
        pts = [(x, y) for y in (-yb, yb) for x in xs]
        return "deck", [{"id": f"s{i + 1}", "at": [r(x), r(y), 0.0], "facing": [0.0, 0.0, 1.0], "interface": "interface.deck-track"} for i, (x, y) in enumerate(pts)], None
    a, b = 0.45 * L, 0.45 * W
    if pat == "ring":
        rad = 0.45 * min(L, W)
        pts = [(rad * math.cos(2 * math.pi * i / n), rad * math.sin(2 * math.pi * i / n)) for i in range(n)]
    elif pat == "trunnion" and n == 3:
        pts = [(a * 0.5, -b), (a * 0.5, b), (-a, 0.0)]
    else:
        pts = rect_points(n, a, b)
    d = bolt_for(att["each"]["tension"], att["each"]["shear"])
    if d is None:
        return "adapter", [], None
    return "adapter", [{"id": f"s{i + 1}", "at": [r(x), r(y), 0.0], "facing": [0.0, 0.0, 1.0], "interface": f"interface.insert-m{d}",
                        "connector": f"stock.st4340-bolt-m{d}"} for i, (x, y) in enumerate(pts)], d


def set_mount(path, holder, socks):
    """Write `holder` and `sockets` into a hand-written mount, keeping its text and comments; replaces what it wrote before."""
    text = open(path, encoding="utf-8").read()
    mark = "# (holder and sockets: written by tools/standards/mounting.py)\n"
    if mark in text:
        head, tail = text.split(mark, 1)
        rest = tail.split("\n# (end of mounting.py)\n", 1)[1] if "\n# (end of mounting.py)\n" in tail else ""
        text = head + rest
    block = yaml.safe_dump({"holder": holder, "sockets": socks}, sort_keys=False, width=230, default_flow_style=None)
    new = text.replace("\nfeeds:", "\n" + mark + block + "# (end of mounting.py)\nfeeds:", 1) if "\nfeeds:" in text else \
        text.replace("\nbasis:", "\n" + mark + block + "# (end of mounting.py)\nbasis:", 1)
    if new != open(path, encoding="utf-8").read():
        if not DRY:
            open(path, "w", encoding="utf-8").write(new)
        return 1
    return 0


# ------------------------------------------------------------------ tubes and cradles
def tubes():
    out = []
    for f, d in load("mill-stock").values():
        i, z = d["identity"], d.get("size") or {}
        mat = d["made_from"][0]["item"]
        if i.get("form") != "tube" or mat not in (AL, ST) or "-S" in i["code"][-6:] or "-D" in i["code"][-6:] or "wall" not in z or z["diameter"] > 0.25 + 1e-9:
            continue
        D, t = z["diameter"], z["wall"]
        area = math.pi / 4 * (D * D - (D - 2 * t) ** 2)
        Z = math.pi * (D ** 4 - (D - 2 * t) ** 4) / (32 * D)
        out.append({"key": i["key"], "name": i["name"], "mat": mat, "D": D, "kgm": area * RHO[mat], "Z": Z})
    return sorted(out, key=lambda t: (t["mat"] != AL, t["kgm"]))


def pick(tl, moment):
    for mat in (AL, ST):
        ok = [t for t in tl if t["mat"] == mat and moment / t["Z"] <= YIELD[mat]]
        if ok:
            return min(ok, key=lambda t: t["kgm"])
    return None


def cradle(slot, m, socks, d, tl):
    e, att = m["envelope"], m["attachment"]
    n, each = att["points"], att["each"]
    xs = [s["at"][0] for s in socks]
    ys = [abs(s["at"][1]) for s in socks]
    Xb = max(1.0, math.ceil(2 * max(xs)) / 2)
    Yb = max(0.5, math.ceil(2 * max(ys)) / 2)
    P = each["compression"] * FACTOR
    cross_x = sorted({round(x, 3) for x in xs} | {-Xb, Xb})
    nfix_beam = int(round(2 * Xb))

    def size(lines):
        """The cross tubes' and beams' worst moments with beams on these lines: a socket between two lines bends its
        cross tube as a point load on that span; each beam takes, at each cross tube, the sockets nearest it, mid-span
        on a 1 m span between fixings."""
        mc, mb = 0.0, 0.0
        for s_ in socks:
            y = s_["at"][1]
            lo = max([l for l in lines if l <= y + 1e-9], default=lines[0])
            hi = min([l for l in lines if l >= y - 1e-9], default=lines[-1])
            g = hi - lo
            mc = max(mc, P * (y - lo) * (hi - y) / g if g > 1e-9 else 0.0)
        for x in cross_x:
            for l in lines:
                q = sum(P for s_ in socks if round(s_["at"][0], 3) == x and min(lines, key=lambda k: abs(k - s_["at"][1])) == l)
                mb = max(mb, q / 4)
        return max(mc, 1e-6), max(mb, 1e-6)

    for lines in ([-Yb, Yb], [r(-Yb + i) for i in range(int(round(2 * Yb)) + 1)]):
        m_cross, m_beam = size(lines)
        tc, tb = pick(tl, m_cross), pick(tl, m_beam)
        if tc and tb:
            break
    else:
        return None
    # End ties are redundant when they overlap a socket-bearing cross tube.
    # Keep socket rows fixed; removing unloaded end ties does not alter size().
    socket_x = sorted({round(x, 3) for x in xs})
    cross_x = sorted(set(socket_x) | {end for end in (-Xb, Xb)
                                   if all(abs(end - x) >= tc["D"] for x in socket_x)})
    h = r(tb["D"] + tc["D"])
    pad_side = 2.5 * d / 1000
    length = 2 * max(Xb, max(abs(x) for x in cross_x) + tc["D"] / 2,
                     max(abs(x) for x in xs) + pad_side / 2)
    width = 2 * max(Yb + tb["D"] / 2, max(ys) + pad_side / 2)
    lb, lc = len(lines) * 2 * Xb, len(cross_x) * 2 * Yb
    pad = (2.5 * d / 1000) ** 2 * (d / 1000) * RHO[ST]
    lug = 0.15 * 0.1 * 0.012 * RHO[AL]
    nfix = len(lines) * nfix_beam
    parts = [{"what": "Beams", "item": tb["mat"], "mass": r(lb * tb["kgm"], 2), "note": f"{len(lines)} of {tb['name']}, {r(2 * Xb, 2)} m each"},
             {"what": "Cross tubes", "item": tc["mat"], "mass": r(lc * tc["kgm"], 2), "note": f"{len(cross_x)} of {tc['name']}, {r(2 * Yb, 2)} m each at x {cross_x}; untrimmed stock mass, fishmouth deductions pending"},
             {"what": "Socket pads and inserts", "item": ST, "mass": r(n * pad, 2), "note": f"{n} 4340 pads, each an M{d} insert; pad thickness {d / 1000:g} m, crosses lowered by that thickness with fishmouth seats to retain socket face z {h:g}; joints unqualified"},
             {"what": "Fixing lugs", "item": AL, "mass": r(nfix * lug, 2), "note": f"{nfix} 6061 lugs, a strut clamp bolted to each"}]
    if slot == "access-s1":
        # Measured reference only; do not spread its cuts across differently sized cradles.
        # Blender cradle/candidate-v01/review/mass-review.json, 2026-10-19.
        assert (n, nfix, cross_x, tb["D"], tc["D"], d) == (8, 6, [-1.485, -1.05, 0.0, 1.05, 1.485], 0.2, 0.25, 42)
        measured = [143.46569443534898, 703.4717793548417, 28.123396788736326, 3.66273821116026]
        notes = [
            "2 hollow 4340 tubes 200 x 5 mm, 3 m; bottom raised 12 mm over fixing plates",
            "5 hollow 4340 tubes 250 x 8 mm, at x [-1.485, -1.05, 0, 1.05, 1.485], fishmouth seats cut; socket face z 0.45 retained",
            "8 4340 pads, 42 mm thick, nominal 42 mm socket bores; detailed threads/inserts unqualified",
            "6 6061 saddle lugs, top z 0.055, 12 mm bottom plates, 46 mm clearance bores for provisional M42 fixings; all fixing datums retained"
        ]
        for part, value, note in zip(parts, measured, notes):
            part["mass"] = r(value, 3)
            part["note"] = note + "; measured from Blender cradle candidate-v01 final 48-segment mesh, 2026-10-19 mass-review.json; joint/load qualification pending"
    mass = r(sum(p["mass"] for p in parts), 3 if slot == "access-s1" else 2)
    ports = [{"id": s["id"], "flow": "in", "medium": "load", "interface": s["interface"], "half": "female", "make": "bolted",
              "at": [s["at"][0], s["at"][1], h], "facing": [0.0, 0.0, 1.0], "optional": True,
              "note": f"the mount's socket, an M{d} insert (bolt {s['connector']}); spare when no foot is on it"} for s in socks]
    share = (m["bears"]["mass"] + mass) / nfix
    k = 0
    for side in lines:
        for i in range(nfix_beam):
            k += 1
            ports.append({"id": f"fix-{k}", "flow": "out", "medium": "load", "interface": "interface.kit-clamp", "half": "male", "make": "bolted",
                          "mass": r(share, 1), "force": r(n * each["compression"] / nfix, 0),
                          "at": [r(-Xb + 0.5 + i), r(side), 0.0], "facing": [0.0, 0.0, -1.0],
                          "note": "clamped onto the strut under it at the middle of a 1 m segment"})
    name = m["identity"]["name"].replace(" mount, ", " cradle, ")
    return {"identity": {"key": f"adapter.{slot}", "name": name,
                         "description": f"Holds what fits {m['identity']['key']} on a ship's frame: {len(lines)} {tb['name']} beams over the frame's struts, 1 m or more apart, clamped at the middle of every 1 m, {len(cross_x)} cross tubes, the mount's {n} sockets on top."},
            "offers": m["identity"]["key"],
            "rating": {"tension": r(n * each["tension"], 0), "compression": r(n * each["compression"], 0), "shear": r(n * each["shear"], 0)},
            "physical": {"length": r(length), "width": r(width), "height": h, "mass": mass},
            "built_of": parts, "ports": ports, "template": "template.cradle",
            "basis": [{"of": ["built_of", "physical", "ports"], "tier": "derived", "review": True,
                       "note": f"By the cradle rule in tools/standards/mounting.py: tubes chosen for the mount's points times {FACTOR} in bending (cross tubes {m_cross / 1e3:.1f} kN m, beams {m_beam / 1e3:.1f} kN m); fixings' duty the mount's mass and points shared evenly, a placeholder: a design's duty comes from the stiffness model. {REVIEW}"},
                      {"of": ["rating"], "tier": "derived", "note": f"The mount's {n} points ({m['identity']['key']})."}]}


# ------------------------------------------------------------------ shared deck support
DECK_INSET = 0.04


def deck_support_shoe():
    parts = [
        {"what": "Crossbar", "item": AL, "mass": 0.5964142370450011,
         "note": "80 x 140 x 20 mm 6061, two nominal M10 seat bores; measured cut volume"},
        {"what": "Fixing boss", "item": AL, "mass": 0.15938280935569557,
         "note": "40 x 40 x 40 mm 6061, nominal M16 blind bore 25 mm deep from bottom; measured cut volume"}
    ]
    ports = [{"id": name, "flow": "in", "medium": "load", "interface": "interface.insert-m10",
              "half": "female", "make": "bolted", "at": [0.0, y, 0.0], "facing": [0.0, 0.0, 1.0],
              "optional": True, "note": "One panel edge per seat. At least one seat must be linked; a perimeter shoe may leave the other spare. Thread/insert and bearing strength unqualified."}
             for name, y in (("panel-a", -DECK_INSET), ("panel-b", DECK_INSET))]
    ports.append({"id": "frame", "flow": "out", "medium": "load", "interface": "interface.kit-clamp",
                  "half": "male", "make": "bolted", "at": [0.0, 0.0, -0.06], "facing": [0.0, 0.0, -1.0],
                  "note": "Required: one strut clamp. Resolve both seat forces and their offset moments at this face, including the 60 mm vertical offset; no stiffness or load rating is approved."})
    return {"identity": {"key": "connector.deck-support-shoe", "name": "Shared deck support shoe",
                         "description": "A separate support beneath a deck edge: two inboard panel seats and one fixing to a strut clamp. Origin midway between the top seat faces, z 0; bottom fixing z -0.06. Rotate about z for each edge. One shoe and one clamp per support, shared by adjacent panels; a perimeter shoe has one spare seat. Bolts and clamp are separate connection hardware, not included in this body's measured mass."},
            "kind": "support-shoe", "size": 0.08, "provides": "adapter",
            "physical": {"length": 0.08, "width": 0.14, "height": 0.06, "mass": r(sum(p["mass"] for p in parts), 6)},
            "built_of": parts, "ports": ports, "template": "template.support-shoe",
            "basis": [{"of": ["physical", "built_of", "ports"], "tier": "derived", "review": True,
                       "note": "Blender deck candidate-v01 shared-shoe-20mm-qc.json, 2026-10-19: cut volume 0.755797 kg, manifold solids, sixteen mating pairs and sampled bolt access clear; actual clamp/head/tool access pending. Ships confirmed separate instance and single-count load path (8765/fc34)."},
                      {"of": ["built_of", "ports"], "tier": "invented", "review": True,
                       "note": "Trial geometry only, no strength rating. Screening cases: 27 kN factored per panel, 54 kN both, 1080 N m single-seat offset. Gross/net crossbar stress 202.5/231.4 MPa against the existing 270 MPa rule. These are not complete service envelopes: resolve asymmetric shear/tension and moments including the 60 mm offset. Thread stripping, concentrations, bearing, welds, fatigue and clamp compliance remain unqualified."}]}


# ------------------------------------------------------------------ deck panels
def deck(L, W):
    tag = f"{L:g}x{W:g}".replace(".", "p")
    area = L * W
    skins, core = 2 * 0.001 * area * RHO[AL], 0.025 * area * 50.0
    nr = max(1, round(W / RAIL_GAP))
    rails = nr * L * 0.6
    segs = int(round(2 * (L + W) / 0.5))
    lugs = segs * 0.15
    parts = [{"what": "Skins", "item": AL, "mass": r(skins, 2), "note": "two 1 mm 6061 sheets"},
             {"what": "Honeycomb core", "item": AL, "mass": r(core, 2), "note": "25 mm aluminium honeycomb, 50 kg/m3"},
             {"what": "Seat track", "item": AL, "mass": r(rails, 2), "note": f"{nr} rails, 0.6 kg/m extruded 6061"},
             {"what": "Edge lugs", "item": AL, "mass": r(lugs, 2), "note": f"{segs} recessed inboard lugs for shared support shoes; inherited 0.15 kg each allowance, final panel cut masses pending"}]
    mass = r(sum(p["mass"] for p in parts), 2)
    th = 0.03
    ports = [{"id": f"track-{k + 1}", "flow": "in", "medium": "load", "interface": "interface.deck-track", "half": "female", "make": "bolted",
              "at": [r(-L / 2 + TRACK_PITCH / 2), r(-W / 2 + RAIL_GAP / 2 + RAIL_GAP * k), th], "facing": [0.0, 0.0, 1.0], "optional": True,
              "note": f"a rail along x: stud i at x + {TRACK_PITCH} i"} for k in range(nr)]
    k = 0
    edges = [((-L / 2, -W / 2), (L / 2, -W / 2)), ((L / 2, -W / 2), (L / 2, W / 2)), ((L / 2, W / 2), (-L / 2, W / 2)), ((-L / 2, W / 2), (-L / 2, -W / 2))]
    for (x0, y0), (x1, y1) in edges:
        n = int(round(math.dist((x0, y0), (x1, y1)) / 0.5))
        for i in range(n):
            f = (i + 0.5) / n
            k += 1
            ports.append({"id": f"edge-{k}", "flow": "out", "medium": "load", "interface": "interface.insert-m10", "half": "male", "make": "bolted",
                          "mass": r(mass / segs, 2), "at": [r(x0 + (x1 - x0) * f - DECK_INSET * (y1 - y0) / math.dist((x0, y0), (x1, y1))), r(y0 + (y1 - y0) * f + DECK_INSET * (x1 - x0) / math.dist((x0, y0), (x1, y1))), 0.0], "facing": [0.0, 0.0, -1.0],
                          "note": "40 mm inboard of the edge, 0.25 m along from a cell corner: male M10 clearance lug onto one seat of connector.deck-support-shoe; the separate shoe attaches to the frame through one strut clamp"})
    return tag, {"identity": {"key": f"panel.deck-{tag}", "name": f"Deck panel, {L:g} x {W:g} m",
                              "description": f"A ship's floor panel on its frame: 6061 skins on aluminium honeycomb, {nr} seat-track rails along it for small equipment's feet; inboard edge lugs bolt to separate shared support shoes, each held by one strut clamp."},
                 "kind": "deck", "site": "ship", "cell": [L, W], "thickness": th, "load": 10000.0, "point_load": STUD["compression"],
                 "ports": ports, "physical": {"length": L, "width": W, "height": th, "mass": mass}, "built_of": parts, "template": "template.deck",
                 "basis": [{"of": ["built_of", "physical", "load", "point_load", "ports"], "tier": "invented", "review": True,
                            "note": f"An aircraft-style sandwich floor and seat track, from memory; 10 kPa and a stud's 18 kN pressed chosen. {REVIEW}"}]}


# ------------------------------------------------------------------ run clips and support spacing
CLIP_BANDS = [0.010, 0.016, 0.025, 0.040, 0.063, 0.100, 0.160, 0.250, 0.400]     # m, the largest run outside each clip takes
PIPE_SPAN = [(0.034, 2.1), (0.060, 3.0), (0.114, 4.3), (0.168, 5.1), (0.219, 5.8), (0.324, 7.0), (0.406, 7.6), (0.610, 9.1)]   # (outside m, span m): water-filled steel at 1 g, the suggested spans of ASME B31.1 from memory
SHIP = {"cable": 0.6, "hose": 0.6}          # m: aircraft practice, wire bundles clamped every 24 in at most (FAA AC 43.13-1B, from memory)
OWN = {"belt": (1.2, "its idler frames"), "drag-conveyor": (3.0, "its casing's supports"), "busbar": (2.0, "its insulator supports"),
       "conductor-rail": (1.5, "its brackets"), "festoon": (1.0, "its trolleys on their track"), "lined-duct": (6.0, "its own steel"), "jacketed-duct": (6.0, "its own steel")}


def run_clip(band):
    mm = round(band * 1000)
    w, t = max(0.012, 0.25 * band), max(0.0015, 0.02 * band)
    loop = math.pi * (band + 0.004) + 2 * (0.02 + band / 4)
    strap = w * t * loop * RHO[AL]
    cushion = math.pi * (band + 0.002) * 0.002 * w * 1100.0
    b = 10 if band <= 0.063 else 16
    hold = 2 * w * t * YIELD[AL]
    rec = {"identity": {"key": f"connector.run-clip-d{mm}", "name": f"Run clip, up to {mm} mm",
                        "description": f"A cushioned 6061 loop clip holding one run up to {mm} mm outside (a cable, a hose, a tube), its tab bolted by an M{b} to a strut clamp's lug: how a run is held on a ship's frame (docs/proposals/physical-mounts.md)."},
           "kind": "run-clip", "size": band, "rating": {"tension": r(hold, 0), "shear": r(MU * hold, 0)},
           "physical": {"length": r(band + 2 * (0.02 + band / 4)), "width": r(band + 2 * t + 0.004), "height": r(w), "mass": r(strap + cushion + bolt_mass(b), 3)},
           "built_of": [{"what": "Strap", "item": AL, "mass": r(strap, 4), "note": f"6061, {round(w * 1000)} x {t * 1000:.1f} mm"},
                        {"what": "Cushion", "item": "material.silicone-rubber", "mass": r(cushion, 4), "note": "2 mm silicone lining"},
                        {"what": "Bolt", "item": ST, "mass": r(bolt_mass(b), 4), "note": f"M{b} with nut, into the strut clamp's lug"}],
           "interface": f"interface.run-clip-d{mm}", "provides": "female", "fixing": "interface.kit-clamp", "template": "template.run-clip",
           "basis": [{"of": ["physical", "rating"], "tier": "invented", "review": True,
                      "note": f"A P-clip by rule (mounting.py): strap a quarter of the band wide, 2% thick; holds both legs at 6061's yield pulled, friction 0.3 sliding. {REVIEW}"}]}
    iface = {"identity": {"key": f"interface.run-clip-d{mm}", "name": f"Run clip, {mm} mm",
                          "description": f"A run's outside (male: any run up to {mm} mm and over the next band down) held in a clip's loop (female)."},
             "domain": "load", "kind": "run-clip", "size": band, "envelope": dict(rec["rating"]), "media": ["load"], "sealing": "open",
             "halves": "male-female", "make": "bolted",
             "basis": [{"of": ["envelope"], "tier": "derived", "note": "The clip's rating."}]}
    return rec, iface


def hold(run):
    """A run's support (ships, 2026-10-18: the registry owns the most spacing by run and support family and service;
    the studio enforces it with load, bend and end restraint; a run with none given is not to be laid there): its clip
    by outside, and the most spacing aboard a ship and in a plant (1 g)."""
    kind = run.get("kind")
    od = max(run.get("outside") or [0.0])
    stands = (run.get("support") or {}).get("spacing")
    if kind in OWN:
        m, by = OWN[kind]
        return {"plant": m, "by": by, "basis": "rule.run-support"}
    out = {}
    band = next((b for b in CLIP_BANDS if od <= b + 1e-9), None)
    if band is not None:
        out["clip"] = f"connector.run-clip-d{round(band * 1000)}"
    if kind in SHIP and band is not None:
        out["ship"] = SHIP[kind]
    if kind == "pipe":
        if band is not None:
            out["ship"] = 0.6 if od <= 0.03 else 1.0 if od <= 0.065 else 1.5 if od <= 0.12 else 2.0
        sp = next((s for o, s in PIPE_SPAN if od <= o + 1e-9), PIPE_SPAN[-1][1])
        out["plant"] = stands or sp
        if band is None:
            out["by"] = "pipe shoes on steel"
    else:
        out["plant"] = stands or {"cable": 1.5, "hose": 1.0}.get(kind, 1.0)
    if stands:
        out["by"] = "its stands (support)"
    out["basis"] = "rule.run-support"
    return out


def stock_for(code, rec, form):
    mats = list(dict.fromkeys(p["item"] for p in rec["built_of"]))
    ph = rec["physical"]
    return {"identity": {"key": "stock." + code.lower(), "code": code, "name": rec["identity"]["name"], "traded_as": "market.machinery", "form": form},
            "package": "package." + PACKAGE_OF.get(form, "crate-heavy"), "made_from": [{"item": m} for m in mats],
            "size": {"length": ph["length"], "width": ph["width"], "height": ph["height"]}, "supplies": {"item": rec["identity"]["key"]},
            "physical": {"mass": ph["mass"]}, "basis": [{"of": ["size", "physical.mass"], "tier": "derived", "note": f"One {rec['identity']['key']}, as made."}]}


# ------------------------------------------------------------------ main
def build():
    faults, n = [], 0
    for d in AS:
        n += write("interfaces", f"insert-m{d}", insert_iface(d), "interface")
    clamps = [clamp(D) for D in KIT_D]
    n += write("interfaces", "kit-clamp", {
        "identity": {"key": "interface.kit-clamp", "name": "Kit strut clamp face",
                     "description": "A holder's fixing (male: a lug with a bolt hole) meeting a kit strut's surface (female, anywhere on its clear length) through a strut clamp sized to the strut (connector.strut-clamp-d<mm>): the clamp's rating and stiffness are the link's."},
        "domain": "load", "kind": "strut-clamp",
        "envelope": {k: max(c["rating"][k] for c, _ in clamps) for k in ("tension", "compression", "shear")},
        "stiffness": {k: min(c["stiffness"][k] for c, _ in clamps) for k in ("axial", "shear")},
        "media": ["load"], "sealing": "open", "halves": "male-female", "make": "bolted",
        "basis": [{"of": ["envelope"], "tier": "derived", "note": "The strongest strut clamp; a link is rated by its own clamp."},
                  {"of": ["stiffness"], "tier": "derived", "review": True, "note": f"The softest strut clamp, where a link names none. {REVIEW}"}]}, "interface")
    n += write("interfaces", "deck-track", {
        "identity": {"key": "interface.deck-track", "name": "Deck seat track",
                     "description": "A deck panel's seat-track rail (female) and an equipment foot's stud (male): studs on a 25.4 mm pitch along the rail, rails 0.25 m apart."},
        "domain": "load", "kind": "track", "envelope": dict(STUD), "stiffness": dict(TRACK_K), "pitch": TRACK_PITCH,
        "media": ["load"], "sealing": "open", "halves": "male-female", "make": "manual",
        "basis": [{"of": ["envelope", "stiffness", "pitch"], "tier": "invented", "review": True, "note": f"The aircraft seat-track convention from memory (25.4 mm pitch); stud ratings and stiffness chosen. {REVIEW}"}]}, "interface")
    for c, b in clamps:
        slug = c["identity"]["key"].split(".", 1)[1]
        n += write("connectors", slug, c, "connector")
    for band in CLIP_BANDS:
        c, i = run_clip(band)
        slug = c["identity"]["key"].split(".", 1)[1]
        n += write("connectors", slug, c, "connector")
        n += write("interfaces", i["identity"]["key"].split(".", 1)[1], i, "interface")
    n += write("rules", "run-support", {"identity": {"key": "rule.run-support", "name": "Run support spacing", "about": "The most spacing between a run's supports, by its family and where it is laid."},
                                         "statement": "Aboard a ship: cable and hose clipped every 0.6 m at most (aircraft practice); tube and pipe every 0.6 m up to 30 mm outside, 1.0 m to 65, 1.5 m to 120, 2.0 m above. In a plant at 1 g: steel pipe at the suggested spans of ASME B31.1 by size (2.1 m at 34 mm to 9.1 m at 610 mm outside), cable 1.5 m, hose 1.0 m; belts, conveyors, busbars, conductor rails, festoons and ducts on their own supports at their family's pitch. A run whose family has no figure for a place is not to be laid there. The studio enforces it, and the clips' load, the run's bend radius and its end restraint.",
                                         "tier": "invented", "source": "FAA AC 43.13-1B (wire clamps 24 in); ASME B31.1 suggested pipe spans; both from memory", "review": True}, "rule")
    shoe = deck_support_shoe()
    n += write("connectors", "deck-support-shoe", shoe, "connector")
    n += write("mill-stock", "KIT-DECK-SUPPORT-SHOE", stock_for("KIT-DECK-SUPPORT-SHOE", shoe, "piece"), "mill-stock")
    tl = tubes()
    cradles, decks = [], []
    for slug, (path, m) in load("mounts").items():
        holder, socks, d = sockets(m)
        if not socks:
            faults.append(f"{m['identity']['key']}: no bolt up to M100 holds a point ({m['attachment']['each']})")
            continue
        n += set_mount(path, holder, socks)
        if holder == "adapter":
            c = cradle(slug, m, socks, d, tl)
            if c is None:
                faults.append(f"{m['identity']['key']}: no kit tube carries its cradle")
                continue
            cradles.append(c)
            n += write("adapters", slug, c, "adapter")
            n += write("mill-stock", "KIT-ADAPTER-" + slug.upper(), stock_for("KIT-ADAPTER-" + slug.upper(), c, "piece"), "mill-stock")
        else:
            decks.append(slug)
    for L, W in DECKS:
        tag, p = deck(L, W)
        n += write("panels", f"deck-{tag}", p, "panel")
        n += write("mill-stock", f"KIT-PANEL-DECK-{tag.upper()}", stock_for(f"KIT-PANEL-DECK-{tag.upper()}", p, "panel"), "mill-stock")
    for t, fam, name, ref, cnt in (("cradle", "adapter", "Equipment cradle", cradles[0]["identity"]["key"] if cradles else None, len(cradles)),
                                   ("deck", "panel", "Deck panel", "panel.deck-1x1", len(DECKS)),
                                   ("strut-clamp", "strut-clamp", "Strut clamp", "connector.strut-clamp-d100", len(clamps)),
                                   ("run-clip", "run-clip", "Run clip", "connector.run-clip-d25", len(CLIP_BANDS)),
                                   ("support-shoe", "support-shoe", "Shared deck support shoe", "connector.deck-support-shoe", 1)):
        n += write("templates", t, {"identity": {"key": f"template.{t}", "name": name, "description": f"{name}: one look for the whole family, scaled to each record's figures (docs/asset-contract.md, Templates; docs/proposals/physical-mounts.md)."},
                                    "family": fam, "draws": "model", "scales_by": ["physical"], "reference": ref, "members": cnt,
                                    **({"model_requests": [{"work": "rig", "because": "Hold v1: exported port-node local +Y does not match registry facing. Correct all 14 frames and verify positions and +Y directions directly from raw GLB world matrices; source/reimport checks alone missed the export-axis conversion.", "asked": "2026-10-19", "of_version": 1, "by": "registry"}]} if t == "cradle" else {}),
                                    "basis": [{"of": ["draws", "scales_by"], "tier": "derived", "note": "The owner, 2026-10-18 (via ships): equipment held by typed mounts on cradles or deck panels made from stock; one template a family."}]}, "template")
    return n, faults, cradles, decks


def main(argv):
    n, faults, cradles, decks = build()
    if DRY and n:
        faults.append(f"{n} record(s) out of date: run tools/standards/mounting.py")
    for f in faults:
        print("fault:", f)
    print(f"{len(cradles)} cradles, {len(decks)} deck mounts, {len(DECKS)} deck panels, {len(KIT_D)} strut clamps; {n} {'stale' if DRY else 'written'}; {len(faults)} fault(s)")
    if "--check" in argv and faults:
        sys.exit(1)


if __name__ == "__main__":
    main(sys.argv)
