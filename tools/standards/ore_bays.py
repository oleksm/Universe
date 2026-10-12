"""The ore bays' weights, worked out from their dimensions, their loads and their materials (standards/SFO/metadata/equipment/
cargo-ore-bay-*.yaml): plate thickness and rib spacing from the ore's pressure, frame tubes from the loaded bay's bending,
then every part as area or length times thickness times density. Rerun after changing a bay's section or length.

    python3 tools/standards/ore_bays.py            # show the working
    python3 tools/standards/ore_bays.py --write    # write mass, parts and the working note into the records

The loads: ore at 2,000 kg/m3 filling the bay at 3 g (29.4 m/s2, the mounts' figure, rule.mounts-margin) toward the gate:
the bay faces thrust, so thrust and a landed world's gravity press the ore toward the gate. Pressure taken as a liquid's at
the full clear depth (loose rock presses its walls less; this errs heavy). Allowed stress two thirds of yield (A36 250 MPa,
4340 1,300 MPa, 6061 270 MPa). The doors carry a reverse 0.3 g (manoeuvring) only. Drives, seals and fasteners are not counted.
"""
import glob, math, os, re, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import edit  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
EQ = os.path.join(ROOT, "standards/SFO/metadata/equipment")
G = 9.80665; A = 3 * G; ORE = 2000; A_DOOR = 0.3 * G
A36, S4340, AL = 7800, 7850, 2700
SIG_A36, SIG_4340, SIG_AL = 250e6 / 1.5, 1300e6 / 1.5, 270e6 / 1.5
T, T_LINER, T_DOOR = 0.008, 0.003, 0.005
CUT = 1.15   # stock bought for a part cut from it (rule.cut-loss)
TUBES = [(60, 2), (80, 3), (100, 5), (120, 10), (150, 10), (200, 15), (250, 20), (300, 30), (400, 30)]   # 4340 tube stock, mm

# The families' sections: outer square, clear width, slot width, clear height (a 45 degree V to the slot under straight sides),
# and how far the gate runs below the shell to clear its webs (measured in the models: 100 t v3, 10 t v2 and the 20 t rebuild).
SECTIONS = {3.15: (2.75, 0.5, 2.7, 0.11), 2.25: (1.93, 0.3, 1.95, 0.065), 1.72: (1.4, 0.3, 1.42, 0.045)}


def tube(d, t):
    d, t = d / 1000, t / 1000
    a = math.pi * (d - t) * t
    return a, a * S4340


def euler(d, t, span):
    """A chord's buckling load between ties, its ends held: pi^2 E I / span^2."""
    d, t = d / 1000, t / 1000
    return math.pi ** 2 * 200e9 * (math.pi / 8 * (d - t) ** 3 * t) / span ** 2


def up(x, step):
    return math.ceil(x / step - 1e-9) * step


def work(rec):
    cap = rec["function"]["capacity"]; vol = rec["function"]["volume"]
    L, W = rec["physical"]["length"], rec["physical"]["width"]
    c, s, H, ds = SECTIONS[W]
    d = (c - s) / 2; hs = H - d; leg = d * math.sqrt(2)
    P = 2 * hs + 2 * leg; sec = c * hs + (c + s) / 2 * d
    Lt = vol / sec
    p = ORE * A * H
    # Plate between ribs, held at the ribs: stress p b^2 / (2 t^2); the spacing the 8 mm plate allows, down to 5 cm.
    b = math.floor(T / math.sqrt(p / (2 * SIG_A36)) / 0.05) * 0.05
    # Ribs: 8 mm webs welded to the plate (the plate their flange, Z about t h^2 / 3), each leg a beam held at both ends.
    span = max(hs, leg); M = p * b * span ** 2 / 12
    h = up(math.sqrt(3 * (M / SIG_A36) / T), 0.01)
    area = P * Lt + 2 * sec
    shell = area * T * A36
    n = math.floor(Lt / b) + 1
    ribs = (n * P + 2 * (math.ceil(c / b) - 1) * 0.8 * H) * h * T * A36
    liners = area * T_LINER * S4340
    # Doors: 5 mm plate, ribbed at the spacing the reverse load allows, each leaf a beam from hinge to latch; 10% hinges.
    # The door ribs hang between the roof ties when shut, so their pitch is a whole number of rib spacings.
    pd = ORE * A_DOOR * H
    bd = min(max(math.floor(T_DOOR / math.sqrt(pd / (2 * SIG_AL)) / b + 1e-9), 1) * b, Lt)
    hd = up(math.sqrt(3 * (pd * bd * (c / 2) ** 2 / 8 / SIG_AL) / T_DOOR), 0.01)
    # Hinges, pins, locks and drives a quarter of the leaves; the steel coaming round the opening they shut on, 8 mm and
    # 50 mm deeper than the door ribs.
    doors = (c * Lt * T_DOOR + (math.floor(Lt / bd) + 1) * c * hd * T_DOOR) * AL * 1.25 + 2 * (Lt + c) * (hd + 0.05) * T * A36
    # Gate: 8 mm plate over the slot and 60 mm onto each bed, ribbed at b across the slot; two 100 mm bearing strips.
    gw = s + 0.12
    hg = up(math.sqrt(3 * (p * b * s ** 2 / 8 / SIG_A36) / T), 0.01)
    # The gate runs below the shell's webs (ds, by section), the slot carried down to it by two skirts, each a cantilever
    # from the slot's edge under the full pressure: 8 mm holds it.
    assert math.sqrt(3 * p * ds ** 2 / SIG_A36) <= T, ds
    # The gate's load to the frame: at every rib a pair of webs as deep as the ribs from the beds out half the V's half-width,
    # and along each side a seat and a guide, 8 mm and 20 mm deeper than the ribs.
    gate = (gw * Lt + n * gw * hg + 2 * Lt * 0.1 + 2 * Lt * ds + 2 * n * h * (c - s) / 4 + 4 * Lt * (h + 0.02)) * T * A36
    # The hopper's load to the frame: every rib carried on down from the V's mid-slope to the bottom chords, two legs a rib.
    ribs += 2 * n * h * T * A36 * (d / 2 + ds + hg)
    ties = n * c * tube(60, 2)[1]
    # Frame: held at its eight corners, the loaded bay bends as a beam over its length; the top chords take the compression.
    m = shell + ribs + liners + doors + gate + ties
    for _ in range(30):
        F = (m + cap) * A * L / 8 / W / 2
        tb = next(x for x in TUBES if tube(*x)[0] >= F / SIG_4340 and euler(*x, b) >= 1.5 * F)
        frame = (4 * L + 2 * 4 * W) * tube(*tb)[1] + ties
        new = shell + ribs + liners + doors + gate + frame
        if abs(new - m) < 0.5:
            break
        m = new
    return dict(cap=cap, L=L, W=W, s=s, Lt=Lt, p=p, b=b, h=h, n=n, ds=ds, bd=bd, hd=hd, hg=hg, tube=tb, F=F, shell=shell, ribs=ribs,
                liners=liners, doors=doors, gate=gate, frame=frame, total=new)


def r10(x):
    return int(round(x / 10) * 10)


def note(w, total):
    return (f"Worked out from its dimensions, loads and materials (tools/standards/ore_bays.py): ore at 2,000 kg/m3 at 3 g "
            f"presses {w['p'] / 1e3:.0f} kPa at the gate (as a liquid at the full clear depth; loose rock presses less). 8 mm A36 "
            f"plate held by ribs every {w['b']:.2f} m (stress p b2 / 2t2 within two thirds of yield), the ribs 8 mm webs "
            f"{w['h'] * 1000:.0f} mm deep carried on down to the bottom chords: shell {w['shell'] / 1e3:.2f} t, ribs {w['ribs'] / 1e3:.2f}; 3 mm 4340 liners over "
            f"all it touches {w['liners'] / 1e3:.2f}; 5 mm aluminium doors ribbed for a reverse 0.3 g, with hinges, locks and the steel coaming they shut on {w['doors'] / 1e3:.2f}; "
            f"an 8 mm ribbed gate on two bearing strips, {w['ds'] * 1000:.0f} mm below the shell on 8 mm skirts that carry the slot down past the webs, with the webs, seats and guides that carry it to the frame, {w['gate'] / 1e3:.2f}; held at its corners, the loaded bay bends as a "
            f"beam, {w['F'] / 1e3:.0f} kN in each top chord: 4340 tube {w['tube'][0]} x {w['tube'][1]} mm with ties at every "
            f"rib (buckling between ties 1.5 times the load or more), frame {w['frame'] / 1e3:.2f}. Loaded by pouring through the open roof: the ties form a "
            f"grate with {w['b'] - 0.06:.2f} m gaps that screens off lumps too big for the {w['s']:.1f} m gate slot (a slot "
            f"passes lumps up to about a third of its width, {w['s'] / 3:.2f} m), so ore is broken to that before loading; the "
            f"door ribs, every {w['bd']:.2f} m, hang between the ties when shut. {total / 1e3:.2f} t, "
            f"{total / w['cap']:.2f} of its capacity; drives, seals and fasteners not counted; the ribs' free edges unchecked "
            f"for buckling. Review.")


def parts(w, rec, prefix):
    L, W, Hh = rec["physical"]["length"], rec["physical"]["width"], rec["physical"]["height"]
    tb = f"stock.st4340-tb-{w['tube'][0]}x{w['tube'][1]}"
    rows = [("Shell", "The hopper's 8 mm plates, sloped to the gate.", w["shell"], "stock.sta36-pl-8", "module.welding-bay"),
            ("Ribs", f"8 mm webs welded round the shell every {w['b']:.2f} m and up the end walls.", w["ribs"], "stock.sta36-pl-8", "module.welding-bay"),
            ("Wear liners", "Hard plates bolted inside where the rock lands and slides.", w["liners"], "stock.st4340-pl-3", "module.cutting-table"),
            ("Top doors", "The pair, hinged at the sides, the length of the roof, that opens to the excavator and shuts for flight; ribbed beneath, the ribs falling between the roof ties.", w["doors"], "stock.al6061-pl-5", "module.welding-bay"),
            ("Discharge gate", "A slot gate the length of the floor, ribbed beneath, sliding on bearing strips along both edges of the slot, that lets the ore out at the dock (its drive not counted); it runs below the shell's webs, the slot carried down to it by two skirts.", w["gate"], "stock.sta36-pl-8", "module.welding-bay"),
            ("Frame", "Tube chords and end frames that carry the hopper to its mount, with a tie across the top at every rib.", w["frame"], tb, "module.welding-bay")]
    total = sum(r10(r[2]) for r in rows)
    out = []
    for i, (name, desc, mass, item, mod) in enumerate(rows, 1):
        k = (r10(mass) / total) ** (1 / 3)
        out.append(f'    - {{ code: {prefix}-{i:02d}, name: "{name}", description: "{desc}", mass: {r10(mass)}, length: {L * k:.3f}, '
                   f'width: {W * k:.3f}, height: {Hh * k:.3f}, item: {item}, quantity: {r10(mass) * CUT:.1f}, module: {mod} }}')
    return total, out


def main(argv):
    for f in sorted(glob.glob(os.path.join(EQ, "cargo-ore-bay-*.yaml")), key=lambda f: int(re.search(r"(\d+)t", f).group(1))):
        rec = yaml.safe_load(open(f)); w = work(rec)
        prefix = rec["built_of"]["list"][0]["code"].split("-")[0]
        total, rows = parts(w, rec, prefix)
        print(f"{w['cap'] / 1e3:>4.0f} t  p {w['p'] / 1e3:3.0f} kPa  ribs {w['b']:.2f} m x {w['h'] * 1000:.0f} mm  tube {w['tube']}  "
              f"shell {w['shell']:6.0f} ribs {w['ribs']:5.0f} liners {w['liners']:5.0f} doors {w['doors']:5.0f} gate {w['gate']:5.0f} "
              f"frame {w['frame']:5.0f}  = {total:6d} kg ({total / w['cap']:.2f})")
        if "--write" in argv:
            def put(s, total=total, rows=rows, w=w):
                s = re.sub(r"^  mass: .*$", f"  mass: {total}", s, count=1, flags=re.M)
                s = re.sub(r"^built_of:\n(?:  .*\n|    .*\n)+", "built_of:\n  parts: " + rec["built_of"]["parts"] + "\n  list:\n" + "\n".join(rows) + "\n", s, count=1, flags=re.M)
                line = f'  - {{ of: [physical.mass, built_of.list], tier: derived, review: true, note: "{note(w, total)}" }}'
                s = re.sub(r"^  - \{ of: \[physical\.mass(?:, built_of\.list)?\], tier: (?:invented|derived).*$", lambda _: line, s, count=1, flags=re.M)
                return s
            edit(f, put)


if __name__ == "__main__":
    main(sys.argv)
