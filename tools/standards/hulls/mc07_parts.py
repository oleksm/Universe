"""Write the MC-07's parts from its model's own objects, each measured (mc07.json from measure.py)."""
import json, os, shutil, sys
r = json.load(open(sys.argv[1])); out = sys.argv[2]
hull = {k: v for k, v in r.items() if k.startswith("Hull_")}
NOSE = min(v["lo"][2] for v in hull.values()); KEEL = min(v["lo"][1] for v in hull.values())
def box(names):
    lo = [min(r[n]["lo"][i] for n in names) for i in range(3)]; hi = [max(r[n]["hi"][i] for n in names) for i in range(3)]
    return lo, hi
def dims(names):
    lo, hi = box(names); return round(hi[2] - lo[2], 2), round(hi[0] - lo[0], 2), round(hi[1] - lo[1], 2)
def area(names): return round(sum(r[n]["area"] for n in names), 1)
def where(names, both=False):
    lo, hi = box(names); c = [(a + b) / 2 for a, b in zip(lo, hi)]
    side = "on the centre line" if abs(c[0]) < 0.05 else f"{abs(c[0]):.1f} m either side of the centre line" if both else f"{abs(c[0]):.1f} m to the {'left' if c[0] < 0 else 'right'} of the centre line"
    return f"Its centre is {c[2] - NOSE:.1f} m back from the nose and {c[1] - KEEL:.1f} m above the keel, {side}."
shutil.rmtree(out, ignore_errors=True); os.makedirs(out)
def write(path, code, name, objs, count, position=None, handed=None, obj_label=None):
    l, w, h = dims(objs)
    t = ["# yaml-language-server: $schema=" + ("../" * (path.count("/") + 3)) + "schema/part.schema.yaml",
         "# Measured from the model (assets/models/mc07.glb), as it stands there: gear down, ramp lowered,", "# clamps and lasers stowed, bay doors shut.",
         "identity:", f"  code: {code}", f"  name: {json.dumps(name)}", "  status: draft", f"  drawing: {json.dumps('assets/models/mc07.glb: ' + (obj_label or ', '.join(objs)))}",
         "physical:", f"  length: {l}", f"  width: {w}", f"  height: {h}", "shape:", f"  surface_area: {area(objs)}"]
    if handed: t.append(f"  handed: {handed}")
    t += ["fit:", f"  count: {count}"]
    if position: t.append(f"  position: {json.dumps(position)}")
    os.makedirs(os.path.dirname(os.path.join(out, path)), exist_ok=True)
    open(os.path.join(out, path), "w").write("\n".join(t) + "\n")
NAMES = {"NoseCap": "Nose cap", "Cockpit": "Cockpit", "CockpitSkirt": "Cockpit skirt", "Cheek_L": "Cheek, left", "Cheek_R": "Cheek, right", "Neck": "Neck", "Shoulder": "Shoulder", "SpineFwd": "Spine, forward",
         "SpineMidA": "Spine, mid A", "SpineMidB": "Spine, mid B", "Main": "Main hull", "MainArmor": "Main armour", "MainArmorTier": "Main armour tier", "MainSponson": "Main sponson", "Keel": "Keel",
         "Connector": "Connector", "RearPod": "Rear pod", "RearModule": "Rear module", "PodArmor": "Pod armour", "PodSponson": "Pod sponson", "EngineBlock": "Engine block", "EngineCowl": "Engine cowl"}
blocks = sorted(hull, key=lambda k: ((hull[k]["lo"][2] + hull[k]["hi"][2]) / 2, -(hull[k]["lo"][1] + hull[k]["hi"][1]) / 2, k))
n = 0
for k in blocks:
    n += 1; short = k[5:]
    write(f"MC07-{n:02d}.yaml", f"MC07-{n:02d}", NAMES[short], [k], 1, where([k]), handed="left" if short.endswith("_L") else "right" if short.endswith("_R") else None)
def assembly(name, objs_one, count, position, subs, label):
    global n
    n += 1; code = f"MC07-{n:02d}"
    write(f"{code}.yaml", code, name, objs_one, count, position, obj_label=label)
    for i, (sname, sobjs, scount) in enumerate(subs, 1):
        write(f"{code}/{code}-{i:03d}.yaml", f"{code}-{i:03d}", sname, sobjs[:1], scount, obj_label=", ".join(sobjs))
leg = lambda p: [f"Gear_{p}_{x}" for x in ("Strut", "Piston", "Pad", "ActBody", "ActRod", "DoorIn", "DoorOut")]
legsub = lambda p: [("Strut", [f"Gear_{p}_Strut"], 1), ("Piston", [f"Gear_{p}_Piston"], 1), ("Pad", [f"Gear_{p}_Pad"], 1), ("Actuator body", [f"Gear_{p}_ActBody"], 1), ("Actuator rod", [f"Gear_{p}_ActRod"], 1), ("Inner door", [f"Gear_{p}_DoorIn"], 1), ("Outer door", [f"Gear_{p}_DoorOut"], 1)]
assembly("Landing leg, front", leg("FL"), 2, where(leg("FL"), True), legsub("FL"), "Gear_FL_*, Gear_FR_*")
assembly("Landing leg, rear", leg("RL"), 2, where(leg("RL"), True), legsub("RL"), "Gear_RL_*, Gear_RR_*")
ramp = ["CargoRamp", "CargoRam_L_Body", "CargoRam_L_Rod", "CargoRam_R_Body", "CargoRam_R_Rod"]
assembly("Cargo ramp", ramp, 1, where(ramp), [("Ramp", ["CargoRamp"], 1), ("Ram body", ["CargoRam_L_Body", "CargoRam_R_Body"], 2), ("Ram rod", ["CargoRam_L_Rod", "CargoRam_R_Rod"], 2)], "CargoRamp, CargoRam_*")
n += 1; write(f"MC07-{n:02d}.yaml", f"MC07-{n:02d}", "Ore bay door, left", ["OreBay_DoorL"], 1, where(["OreBay_DoorL"]), handed="left")
n += 1; write(f"MC07-{n:02d}.yaml", f"MC07-{n:02d}", "Ore bay door, right", ["OreBay_DoorR"], 1, where(["OreBay_DoorR"]), handed="right")
clamp = lambda p: [f"Clamp{p}_{x}" for x in ("Pad", "Stage1", "Stage2", "Stage3", "Jaw0", "Jaw1", "Jaw2", "Jaw3")]
cf, ca = box(clamp("Fwd")), box(clamp("Aft"))
assembly("Clamp", clamp("Fwd"), 2, f"On the centre line, on top of the spine: one {(cf[0][2] + cf[1][2]) / 2 - NOSE:.1f} m back from the nose, one {(ca[0][2] + ca[1][2]) / 2 - NOSE:.1f} m back.",
         [("Pad", ["ClampFwd_Pad"], 1), ("Stage 1", ["ClampFwd_Stage1"], 1), ("Stage 2", ["ClampFwd_Stage2"], 1), ("Stage 3", ["ClampFwd_Stage3"], 1), ("Jaw", ["ClampFwd_Jaw0", "ClampFwd_Jaw1", "ClampFwd_Jaw2", "ClampFwd_Jaw3"], 4)], "ClampFwd_*, ClampAft_*")
laser = lambda p: [f"Laser{p}_{x}" for x in ("Lift", "Gimbal", "Head", "Hatch")]
assembly("Mining laser", laser("L"), 2, where(laser("L"), True), [("Lift", ["LaserL_Lift"], 1), ("Gimbal", ["LaserL_Gimbal"], 1), ("Head", ["LaserL_Head"], 1), ("Hatch", ["LaserL_Hatch"], 1)], "LaserL_*, LaserR_*")
print(n, "parts at the top;", sum(len(fs) for _, _, fs in os.walk(out)) - n, "under them; nose z", round(NOSE, 2), "keel y", round(KEEL, 2))

# Mass, as an estimate to be balanced later: the hull's frame mass (the game's figure, kg) shared
# over the parts that are not made of other parts, by their surface.
import glob, re
FRAME = 87803.0
files = sorted(glob.glob(os.path.join(out, "**", "*.yaml"), recursive=True))
text = {f: open(f).read() for f in files}
num = lambda f, key: float(re.search(rf"^\s*{key}: ([0-9.]+)", text[f], re.M).group(1))
leaf = [f for f in files if not os.path.isdir(f[:-5])]
each = lambda f: num(f, "count") * (num(os.path.dirname(f) + ".yaml", "count") if os.path.basename(os.path.dirname(f)) != os.path.basename(out) else 1)
per_m2 = FRAME / sum(num(f, "surface_area") * each(f) for f in leaf)
for f in leaf:
    t = text[f].replace("# clamps and lasers stowed, bay doors shut.\n", "# clamps and lasers stowed, bay doors shut. Its mass is an estimate, to be balanced: its surface\n# at %.2f kg for each m2 (the hull's frame mass in the game shared over all the parts' surface).\n" % per_m2)
    t = t.replace("physical:\n", "physical:\n  mass: %d\n" % round(num(f, "surface_area") * per_m2))
    open(f, "w").write(t)
print("mass: %.2f kg for each m2, over %d parts" % (per_m2, len(leaf)))
