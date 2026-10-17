"""Fit check of a layout snapped together from its records (SFO 25): units' boxes at their bays, runs along their paths,
the frame's faces and openings. No model is needed: this is the check blender's assembly did by eye.

    python3 tools/standards/fit_layout.py [layout.<name> ...]      (default: every layout with runs)

Faults: a run's end not at its port, or not leaving it the way it faces; ends of the wrong connector, flow or medium;
a tee or tap not on its run's path; legs too short for the run's elbows; a run through a unit, another run or a frame's
face outside its openings; ports left open; power needed and not fed. Exit 1 when there is any.
"""

import glob
import math
import os
import sys

import numpy as np
import yaml

S = "standards/SFO/metadata/"
STEP = 0.25        # m between the points a run is sampled at
GAP = 0.05         # m kept clear between runs
HEADROOM = 4.5     # m a run keeps clear over a road (a forklift's mast, a truck: from memory)
CULVERT = 0.3      # m of cover a run under a road keeps between its top and the road's surface
HOOD_GAP = 0.15    # m: a duct's catch hood over a moving outlet (plant_kit.GAP_HOOD)
TOUCH = 0.03       # m: a run touching a unit's box closer than this is seated on it, not through it (a gallery's foil on a deck)
DROP = 0.5         # m a belt's head stands over the belt it drops onto (plant_kit.TRANSFER_DROP)


def load(folder):
    out = {}
    for f in glob.glob(S + folder + "/*.yaml"):
        d = yaml.safe_load(open(f, encoding="utf-8"))
        out[d["identity"]["key"]] = d
    return out


def world(bay, local):
    a = math.radians(bay.get("turn", 0.0))
    x, y, z = local
    return np.array([bay["at"][0] + x * math.cos(a) - y * math.sin(a), bay["at"][1] + x * math.sin(a) + y * math.cos(a), bay["at"][2] + z])


def turn_vec(bay, v):
    a = math.radians(bay.get("turn", 0.0))
    return np.array([v[0] * math.cos(a) - v[1] * math.sin(a), v[0] * math.sin(a) + v[1] * math.cos(a), v[2]])


def end_offset(conn_kind, run, which, face=(0.0, 0.0, 1.0)):
    if conn_kind == "belt-transfer" and which == 1:
        return np.array([0.0, 0.0, 3.0])
    if conn_kind == "hood":
        return HOOD_GAP * np.asarray(face, float)
    if conn_kind == "chute":
        return np.array([0.0, 0.0, -run["outside"][1] / 2 if which == 0 else run["outside"][1] / 2])
    return np.zeros(3)


def seg_dist(p, a, b):
    """Distance from points p (n, 3) to segment a-b."""
    ab = b - a
    t = np.clip(((p - a) @ ab) / max(ab @ ab, 1e-12), 0.0, 1.0)
    return np.linalg.norm(p - (a + t[:, None] * ab), axis=1)


def path_of(link):
    return np.array([link["from"], *link.get("via", []), link["to"]], dtype=float)


def samples(pts):
    out = []
    for a, b in zip(pts, pts[1:]):
        n = max(1, int(math.ceil(np.linalg.norm(b - a) / STEP)))
        out.append(a + np.linspace(0, 1, n + 1)[:, None] * (b - a))
    return np.vstack(out)


def check(lay, units, runs, conns):
    faults, notes = [], []
    bays = {b["id"]: b for b in lay["bays"]}
    edge = {p["id"]: p for p in lay.get("ports", [])}
    links = [l for l in lay["links"] if l.get("run")]
    by_name = {l["name"]: l for l in links}
    used = set()
    radius, half = {}, {}

    def port_of(ref):
        bid, pid = ref.split(":")
        b = bays.get(bid)
        if not b:
            return None, None
        u = units[b["unit"]]
        return b, next((p for p in u.get("ports", []) if p["id"] == pid), None)

    # Ports on the units placed: inside their boxes, built to a known connector.
    for b in lay["bays"]:
        u = units[b["unit"]]
        L, W, H = u["physical"]["length"], u["physical"]["width"], u["physical"]["height"]
        lo_, hi_ = np.array([-L / 2, -W / 2, 0.0]), np.array([L / 2, W / 2, H])
        ext = [e_ for e_ in u.get("envelope") or []]
        for s_ in u.get("shells") or []:           # (a unit whose frame is not centred, as a hall's head: its measured extent)
            x0, x1_, yc, zc, ri, ro = s_["at"]
            ext.append([x0, yc - ro, zc - ro, x1_ if s_["kind"] == "tube" else x0 + x1_, yc + ro, zc + ro])
        if ext:
            lo_, hi_ = np.minimum(lo_, np.min([e_[:3] for e_ in ext], axis=0)), np.maximum(hi_, np.max([e_[3:] for e_ in ext], axis=0))
        for p in u.get("ports", []):
            x, y, z = p["at"]
            if not all(lo_[k] - 0.5 <= v <= hi_[k] + 0.5 for k, v in enumerate((x, y, z))):
                faults.append(f"{b['unit']}: port {p['id']} at {p['at']} is outside its box")
            if p.get("connector") and p["connector"] not in conns:
                faults.append(f"{b['unit']}: port {p['id']}: no connector {p['connector']}")

    for l in links:
        run = runs.get(l["run"])
        if not run:
            faults.append(f"{l['name']}: no run {l['run']}")
            continue
        pts = path_of(l)
        w = run["outside"][0] * (math.sqrt(l.get("count", 1)))
        radius[l["name"]] = max(w, run["outside"][1]) / 2
        half[l["name"]] = (w / 2, run["outside"][1] / 2)          # (across and up: against units' boxes a run is its section, not its round)
        R = run.get("elbow", {}).get("radius", 0.0)
        items = []
        for which, e in enumerate(l.get("ends", [])):
            at = pts[0] if which == 0 else pts[-1]
            leave = (pts[1] - pts[0]) if which == 0 else (pts[-2] - pts[-1])
            leave = leave / max(np.linalg.norm(leave), 1e-12)
            if e == "cap":
                continue
            if e.startswith("tee:"):
                main = by_name.get(e[4:])
                if not main:
                    faults.append(f"{l['name']}: tees off {e[4:]}, which is not a run here")
                    continue
                mp = path_of(main)
                d = min(seg_dist(at[None], a, b)[0] for a, b in zip(mp, mp[1:]))
                if d > 0.02:
                    faults.append(f"{l['name']}: its tee at {at.tolist()} is {d:.2f} m off {main['name']}")
                rm = runs[main["run"]].get("elbow", {}).get("radius", 0.0)
                for c in mp[1:-1]:
                    if np.linalg.norm(at - c) < rm:
                        faults.append(f"{l['name']}: its tee on {main['name']} is in that run's elbow at {c.tolist()}")
                if runs[main["run"]]["carries"] != run["carries"]:
                    faults.append(f"{l['name']}: carries {run['carries']}, tees off {main['name']} which carries {runs[main['run']]['carries']}")
                continue
            if e.startswith("jacket:"):               # (a pipe ending on a jacketed duct's water jacket)
                main = by_name.get(e[7:])
                if not main or runs.get(main.get("run"), {}).get("kind") != "jacketed-duct":
                    faults.append(f"{l['name']}: ends on {e[7:]}'s jacket, which is not a jacketed duct here")
                    continue
                if run["kind"] != "pipe":
                    faults.append(f"{l['name']}: only a pipe ends on a duct's jacket")
                mp = path_of(main)
                d = min(seg_dist(at[None], a, b)[0] for a, b in zip(mp, mp[1:]))
                if d > 0.02:
                    faults.append(f"{l['name']}: its end at {at.tolist()} is {d:.2f} m off {main['name']}")
                continue
            if e.startswith("onto:"):                 # (a belt's head over another belt, TRANSFER_DROP above its path)
                main = by_name.get(e[5:])
                if not main:
                    faults.append(f"{l['name']}: drops onto {e[5:]}, which is not a run here")
                    continue
                if (which != 1 and not l.get("reversible")) or run["carries"] != "bulk" or runs[main["run"]]["carries"] != "bulk":
                    faults.append(f"{l['name']}: only a bulk run's to end drops onto another bulk run")
                mp = path_of(main)
                d = min(seg_dist((at - np.array([0.0, 0.0, DROP]))[None], a, b)[0] for a, b in zip(mp, mp[1:]))
                if d > 0.02:
                    faults.append(f"{l['name']}: its head at {at.tolist()} is not {DROP:g} m over {main['name']} ({d:.2f} m off)")
                continue
            if e.startswith("edge:"):
                p = edge.get(e[5:])
                if not p:
                    faults.append(f"{l['name']}: no layout port {e}")
                    continue
                pos, face, flow = np.array(p["at"], float), np.array(p["facing"], float), {"in": "out", "out": "in", "both": "both"}[p["flow"]]
            else:
                b, p = port_of(e)
                if not p:
                    faults.append(f"{l['name']}: no port {e}")
                    continue
                pos, face, flow = world(b, p["at"]), turn_vec(b, p["facing"]), p["flow"]
            used.add(e)
            ck = conns[p["connector"]]["kind"] if p["connector"] in conns else None
            pos = pos + end_offset(ck, run, 1 if l.get("reversible") else which, face)
            if np.linalg.norm(pos - at) > 0.02:
                faults.append(f"{l['name']}: its {'from' if which == 0 else 'to'} end {at.tolist()} is not at {e} ({pos.round(3).tolist()})")
            if p["connector"] != run["connector"] and not (run["kind"] == "belt" and ck == "chute") and not (run["kind"] == "jacketed-duct" and ck == "hood") and not (run["kind"] == "lined-duct" and ck == "clamp-coupling"):     # (a chute drops onto a belt's tail, into its skirt; a belt's head drops through its hood into a chute's mouth)
                faults.append(f"{l['name']}: {run['connector']} at its end, {e} is built to {p['connector']}")
            want = "in" if l.get("reversible") else ("out" if which == 0 else "in")
            if flow not in (want, "both"):
                faults.append(f"{l['name']}: its {'from' if which == 0 else 'to'} end is {e}, which flows {p['flow']}")
            if p["medium"] != run["carries"] and not (run["kind"] == "pipe" and {p["medium"], run["carries"]} <= {"gas", "liquid"}):     # (a pipe class carries either; its supports are sized full of water)
                faults.append(f"{l['name']}: carries {run['carries']}, {e} is {p['medium']}")
            items.append(p.get("item"))
            dot = float(leave @ face)
            need = {"flange": 0.7, "lined-flange": 0.7, "hood": 0.7, "clamp-coupling": 0.7, "chute": -0.35}.get(ck)     # (a belt may rise from under a chute at up to 20 degrees, its loading zone's limit; from memory)
            if need is not None and dot < need:
                faults.append(f"{l['name']}: leaves {e} at {math.degrees(math.acos(max(-1, min(1, dot)))):.0f} deg from the way it faces")
        if len(items) == 2 and None not in items and items[0] != items[1]:
            faults.append(f"{l['name']}: {items[0]} at one end, {items[1]} at the other")
        for t in l.get("taps", []):
            b, p = port_of(t)
            if not p:
                faults.append(f"{l['name']}: no tap port {t}")
                continue
            used.add(t)
            w_ = world(b, p["at"])
            if run["carries"] == "bulk":              # (a short chute or plough between the port and the belt, the run's: up to 1.6 m aside, 1.5 m under or over its path)
                best = min(((seg_dist(w_[None], a, c)[0], a, c) for a, c in zip(pts, pts[1:])), key=lambda x: x[0])
                a, c = best[1], best[2]
                u_ = (c - a) / max(np.linalg.norm(c - a), 1e-12)
                k_ = a + np.clip((w_ - a) @ u_, 0, np.linalg.norm(c - a)) * u_
                dz, dh = w_[2] - k_[2], math.hypot(w_[0] - k_[0], w_[1] - k_[1])
                if not (-1.5 - 1e-3 <= dz <= 1.5 + 1e-3 and dh <= 1.6 + 1e-3):
                    faults.append(f"{l['name']}: tap {t} is {dh:.2f} m aside and {dz:+.2f} m from its path (a chute reaches 1.6 aside, 1.5 under or over)")
            else:
                d = min(seg_dist(w_[None], a, c)[0] for a, c in zip(pts, pts[1:]))
                if d > 0.02:
                    faults.append(f"{l['name']}: tap {t} is {d:.2f} m off its path")
            if p["medium"] != run["carries"]:
                faults.append(f"{l['name']}: tap {t} is {p['medium']}, the run carries {run['carries']}")
        # Legs long enough for the elbows at their ends.
        legs = [float(np.linalg.norm(b - a)) for a, b in zip(pts, pts[1:])]
        if run["kind"] == "belt" and len(pts) > 2:
            faults.append(f"{l['name']}: a belt runs straight; it turns {len(pts) - 2} times")
        vert = [abs(b[2] - a[2]) > 0.5 * max(np.linalg.norm(b - a), 1e-9) for a, b in zip(pts, pts[1:])]
        Rc = [run.get("riser", {}).get("radius", R) if (vert[i - 1] or vert[i]) else R for i in range(1, len(pts) - 1)]    # (a cable's bend into a rise is a riser)
        for i, leg in enumerate(legs):
            need = (Rc[i - 1] if i > 0 else 0) + (Rc[i] if i < len(legs) - 1 else 0)
            if R and leg + 1e-3 < need:
                faults.append(f"{l['name']}: leg {i + 1} is {leg:.2f} m, its elbows need {need:.2f}")

    # Clashes: runs through units (other than those they join), through each other, through the frame's faces.
    S_ = {l["name"]: samples(path_of(l)) for l in links if l["name"] in radius}
    for l in links:
        if l["name"] not in radius:
            continue
        taps = {t.split(":")[0] for t in l.get("taps", [])}          # (a manifold under the units it taps, by design)
        joined = {}                                                    # the units at its ends: only the stretch at the port is let through
        for which, e in enumerate(l.get("ends", [])):
            if ":" in e and not e.startswith(("tee:", "edge:", "jacket:", "onto:")):
                joined.setdefault(e.split(":")[0], []).append(path_of(l)[0 if which == 0 else -1])
        R_ = runs[l["run"]].get("elbow", {}).get("radius", 0.0)
        pts, rad = S_[l["name"]], radius[l["name"]]
        rw, rh = half[l["name"]]
        for b in lay["bays"]:
            if b["id"] in taps and not (units[b["unit"]].get("envelope") or units[b["unit"]].get("shells")):
                continue
            q_ok = np.zeros(len(pts), bool)
            for e_at in joined.get(b["id"], []):
                q_ok |= np.linalg.norm(pts - e_at, axis=1) < rad + R_ + 2.5
            u = units[b["unit"]]
            Lb, Wb, Hb = u["physical"]["length"], u["physical"]["width"], u["physical"]["height"]
            a = math.radians(b.get("turn", 0.0))
            q = pts - np.array(b["at"], float)
            lx = q[:, 0] * math.cos(a) + q[:, 1] * math.sin(a)
            ly = -q[:, 0] * math.sin(a) + q[:, 1] * math.cos(a)
            for t in l.get("taps", []):          # (a run passing through a unit's nozzle or opening it taps: let through at the opening)
                if t.split(":")[0] == b["id"]:
                    tb, tp_ = port_of(t)
                    if tp_:
                        q_ok |= np.linalg.norm(pts - world(tb, tp_["at"]), axis=1) < rad + 2.0
            if u.get("envelope") or u.get("shells"):          # (the boxes measured on its model, and a hollow unit's shells; else its whole box)
                hit = np.zeros(len(pts), bool)
                for e_ in (u.get("envelope") or []) + [{s_["kind"]: s_["at"]} for s_ in u.get("shells") or []]:
                    if isinstance(e_, dict) and "tube" in e_:
                        x0, x1, yc, zc, ri, ro = e_["tube"]
                        rho = np.hypot(ly - yc, q[:, 2] - zc)
                        hit |= (lx > x0 - rw) & (lx < x1 + rw) & (rho > ri - rad) & (rho < ro + rad)
                    elif isinstance(e_, dict) and "dome" in e_:
                        x0, dep, yc, zc, ri, ro = e_["dome"]
                        rho = np.hypot(ly - yc, q[:, 2] - zc)
                        f = np.hypot(np.maximum(lx - x0, 0.0) / dep, rho / ro)
                        hit |= (lx > x0 - rad) & (f > (ri - rad) / ro) & (f < 1.0 + rad / ro)
                    else:
                        x0, y0, z0, x1, y1, z1 = e_
                        rw_, rh_ = rw - TOUCH, rh - TOUCH        # (a face on a face, as a gallery sealed to a lock's outlet, is not a clash)
                        hit |= (lx > x0 - rw_) & (lx < x1 + rw_) & (ly > y0 - rw_) & (ly < y1 + rw_) & (q[:, 2] > z0 - rh_) & (q[:, 2] < z1 + rh_)
                hit &= ~q_ok
            else:
                hit = (np.abs(lx) < Lb / 2 + rw) & (np.abs(ly) < Wb / 2 + rw) & (q[:, 2] > -rh) & (q[:, 2] < Hb + rh) & ~q_ok
            if hit.any():
                k = int(np.argmax(hit))
                faults.append(f"{l['name']}: through {b['id']} ({b['unit']}) at {pts[k].round(2).tolist()}")
    # Moving units: each one's boxes swept along the rail it rides, end to end; no run may stand in that path (a
    # gantry crane's legs sweep the whole length of its rails). Rails are straight.
    rails = {l["name"]: l for l in lay["links"] if l.get("kind") == "rail"}
    for m_ in lay.get("mobile", []):
        rl, u = rails.get(m_["rides"]), units.get(m_["unit"])
        if not rl or not u:
            continue
        a_, b_ = np.array(rl["from"], float), np.array(rl["to"], float)
        d_ = (b_ - a_) / np.linalg.norm(b_ - a_)
        at_ = np.array(m_["at"], float)
        off = at_ - a_ - ((at_ - a_) @ d_) * d_            # (where it stands across its rail)
        Lm, Wm, Hm = u["physical"]["length"], u["physical"]["width"], u["physical"]["height"]
        boxes = u.get("envelope") or [[-Lm / 2, -Wm / 2, 0.0, Lm / 2, Wm / 2, Hm]]
        swept = []
        for e_ in boxes:
            lo, hi = np.array(e_[:3], float), np.array(e_[3:], float)
            swept.append((np.minimum(a_ + off + lo, b_ + off + lo), np.maximum(a_ + off + hi, b_ + off + hi)))
        for l in links:
            if l["name"] not in radius:
                continue
            pts = S_[l["name"]]
            rw, rh = half[l["name"]]
            for lo, hi in swept:
                pad = np.array([rw, rw, rh]) - TOUCH
                inside = np.all((pts > lo - pad) & (pts < hi + pad), axis=1)
                if inside.any():
                    faults.append(f"{l['name']}: in the path of {m_['unit']} riding {m_['rides']}, at {pts[np.argmax(inside)].round(2).tolist()}")
                    break
    # Roads: a run over one keeps HEADROOM clear above it, or passes under it in a culvert; no unit stands on it.
    for rd in [l for l in lay["links"] if l.get("kind") == "road" and l.get("width")]:
        P = path_of(rd)
        hw = rd["width"] / 2
        zr = float(P[:, 2].max())
        flat = lambda q: np.c_[q[:, :2], np.zeros(len(q))]
        Pf = flat(P)
        plan = lambda q: np.min([seg_dist(flat(q), a, b) for a, b in zip(Pf, Pf[1:])], axis=0)
        for l in links:
            if l["name"] not in radius:
                continue
            pts = S_[l["name"]]
            rw, rh = half[l["name"]]
            bad = (plan(pts) < hw + rw - TOUCH) & (pts[:, 2] - rh < zr + HEADROOM) & (pts[:, 2] + rh > zr - CULVERT)
            if bad.any():
                q = pts[np.argmax(bad)]
                faults.append(f"{l['name']}: over {rd['name']} at {q.round(2).tolist()}, {q[2] - rh - zr:.2f} m clear (a road wants {HEADROOM:g} m, or a culvert under it)")
        for b in lay["bays"]:
            u = units[b["unit"]]
            Lb, Wb = u["physical"]["length"], u["physical"]["width"]
            a = math.radians(b.get("turn", 0.0))
            g = np.array([(x, y) for x in np.linspace(-Lb / 2, Lb / 2, max(2, int(Lb / 0.5) + 1)) for y in np.linspace(-Wb / 2, Wb / 2, max(2, int(Wb / 0.5) + 1))])
            wpts = np.c_[b["at"][0] + g[:, 0] * math.cos(a) - g[:, 1] * math.sin(a), b["at"][1] + g[:, 0] * math.sin(a) + g[:, 1] * math.cos(a), np.zeros(len(g))]
            if b["at"][2] < zr + HEADROOM and (plan(wpts) < hw - TOUCH).any():
                faults.append(f"{b['id']} ({b['unit']}) stands on {rd['name']}")
    names = [l["name"] for l in links if l["name"] in radius]
    for i, na in enumerate(names):
        la = by_name[na]
        for nb in names[i + 1:]:
            lb = by_name[nb]
            tees = [(x, y) for x, y in ((la, lb), (lb, la)) if any(f"{k}:{y['name']}" in x.get("ends", []) for k in ("tee", "onto", "jacket"))]
            pb = path_of(lb)
            d = np.full(len(S_[na]), np.inf)
            for a, b in zip(pb, pb[1:]):
                d = np.minimum(d, seg_dist(S_[na], a, b))
            bad = d < radius[na] + radius[nb] + GAP
            for x, y in tees:
                tp = path_of(x)[0] if x.get("ends", ["", ""])[0] in (f"tee:{y['name']}", f"onto:{y['name']}", f"jacket:{y['name']}") else path_of(x)[-1]
                keep = radius[na] + radius[nb] + runs[y["run"]].get("elbow", {}).get("radius", 0.0) + runs[x["run"]].get("elbow", {}).get("radius", 0.0) + (3.0 if f"onto:{y['name']}" in x.get("ends", []) else 0.5)     # (a belt's head reaches into the receiving belt's hood)
                bad &= np.linalg.norm(S_[na] - tp, axis=1) > keep
            if bad.any():
                k = int(np.argmax(bad))
                faults.append(f"{na}: within {d[k]:.2f} m of {nb} at {S_[na][k].round(2).tolist()} (needs {radius[na] + radius[nb] + GAP:.2f})")
    fr = lay.get("frame") or {}
    if fr.get("box"):
        x0, y0, z0, x1, y1, z1 = fr["box"]
        opens = fr.get("openings", [])
        faces = [(0, x0), (0, x1), (1, y0), (1, y1), (2, z0), (2, z1)]
        lo, hi = np.array([x0, y0, z0]), np.array([x1, y1, z1])
        for l in links:
            if l["name"] not in radius:
                continue
            rad = radius[l["name"]]
            pts = path_of(l)
            for a, b in zip(pts, pts[1:]):
                for ax, v in faces:
                    if (a[ax] - v) * (b[ax] - v) >= 0 or a[ax] == b[ax]:
                        continue
                    c = a + (v - a[ax]) / (b[ax] - a[ax]) * (b - a)
                    oth = [k for k in range(3) if k != ax]
                    inside = all(lo[k] - rad < c[k] < hi[k] + rad for k in oth)
                    if not inside:
                        continue
                    if all(lo[k] + rad <= c[k] <= hi[k] - rad for k in oth) or any(c[k] >= hi[k] + rad or c[k] <= lo[k] - rad for k in oth):
                        pass
                    ok = any(all(o[k] - 1e-6 <= c[k] - rad and c[k] + rad <= o[k + 3] + 1e-6 for k in oth) and o[ax] - 1e-6 <= v <= o[ax + 3] + 1e-6 for o in opens)
                    if not ok:
                        faults.append(f"{l['name']}: through the frame's {'xyz'[ax]} = {v:g} face at {c.round(2).tolist()}, outside its openings")

    # Ports left open; power needed and not fed.
    for b in lay["bays"]:
        u = units[b["unit"]]
        ports = u.get("ports", [])
        if not ports:
            notes.append(f"{b['id']} ({b['unit']}): no ports yet")
            continue
        for p in ports:
            if f"{b['id']}:{p['id']}" not in used and not p.get("optional") and p["medium"] != "load":      # (a load port is joined by the bay it stands in)
                faults.append(f"{b['id']}:{p['id']} ({b['unit']}) is left open")
        if (u.get("needs") or {}).get("power") and not any(p["medium"] == "power" and f"{b['id']}:{p['id']}" in used for p in ports):
            faults.append(f"{b['id']} ({b['unit']}) needs {u['needs']['power'] / 1e3:,.0f} kW and nothing feeds it")
    for pid in edge:
        if f"edge:{pid}" not in used:
            faults.append(f"edge:{pid} is left open")
    old = [l["name"] for l in lay["links"] if not l.get("run") and l.get("kind") not in ("rail", "road")]     # (rails and roads are laid as they are, not of runs)
    if old:
        notes.append(f"{len(old)} links not laid of runs yet: {', '.join(old[:6])}{'...' if len(old) > 6 else ''}")
    return faults, notes


def main():
    os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
    layouts, units, runs, conns = load("layouts"), load("units"), load("runs"), load("connectors")
    want = sys.argv[1:] or [k for k, d in sorted(layouts.items()) if any(l.get("run") for l in d.get("links", []))]
    bad = 0
    for k in want:
        f, n = check(layouts[k], units, runs, conns)
        lay = layouts[k]
        laid = sum(1 for l in lay["links"] if l.get("run"))
        print(f"{k}: {len(lay['bays'])} bays, {laid} runs, {len(lay.get('ports', []))} layout ports: {len(f)} fault(s)")
        for x in f:
            print("  fault:", x)
        for x in n:
            print("  note:", x)
        bad += len(f)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
