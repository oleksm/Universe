"""Install a planet simulation's world (its survey, energy and surface packages) into the registry (docs/survey-contract.md).

    python3 tools/standards/world_install.py <world folder or its survey folder> [<body key>] [--replace]

The body key is read from the survey's summary.json when it names one. One command, idempotent: run it again on the same
world and nothing changes; run it on a new version of the surface bake and only the pointer moves. Then build.py.

Beside the survey, the world's other packages are taken when they sit next to it (worlds/<id>/energy/, worlds/<id>/surface/):
the energy package is copied whole (small) and summarised on the record (`energy`); the surface package is large and is
not copied: its latest.json and manifest.json are, and the record points at it (`bake`) for the game to fetch by hash.

Checks the survey (format, immutable, every file's SHA-256 against the manifest), copies it to
standards/Celestial/surveys/<world_id>/ (read-only), points the body's record at it, and takes the
derived figures the survey owns (land share, mean temperature where given) into the record, dropping
the seed's guesses for them. The body becomes provenance `baked`. A body already baked is left alone
unless --replace is given: a new run is a new world, and claims and mines point at the old one's IDs.
Then run build.py so the page and the tracker see it. Run from the repository root.
"""
import hashlib, json, os, shutil, stat, sys, re
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SURVEYS = os.path.join(ROOT, "standards", "Celestial", "surveys")
FILES = ("manifest.json", "summary.json", "deposits.geojson", "districts.json", "bulk_rock.json", "geology.png", "rock_units.png")
FORMAT = "planet-sim-survey/1"


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def check(folder):
    """The survey is whole and is what its manifest says. Returns the manifest and the summary."""
    for name in FILES:
        if not os.path.isfile(os.path.join(folder, name)):
            sys.exit(f"survey: {name} missing in {folder}")
    m = json.load(open(os.path.join(folder, "manifest.json"), encoding="utf-8"))
    if m.get("format") != FORMAT:
        sys.exit(f"survey: format {m.get('format')!r}, want {FORMAT!r}")
    if not m.get("immutable"):
        sys.exit("survey: manifest does not say immutable: not a finished survey")
    for name, want in m["files"].items():
        got = sha(os.path.join(folder, name))
        if got != want["sha256"]:
            sys.exit(f"survey: {name} does not match its manifest ({got[:12]} vs {want['sha256'][:12]})")
    return m, json.load(open(os.path.join(folder, "summary.json"), encoding="utf-8"))


def body_file(key):
    for dirpath, _, files in os.walk(os.path.join(ROOT, "standards", "Celestial", "metadata", "systems")):
        for f in files:
            if f.endswith(".yaml") and os.path.basename(dirpath) == "bodies":
                p = os.path.join(dirpath, f)
                if (yaml.safe_load(open(p, encoding="utf-8")) or {}).get("identity", {}).get("key") == key:
                    return p
    sys.exit(f"no body {key}")


def place(folder, world_id):
    """Copy the survey into the registry, read-only; refuse to overwrite a different one."""
    dest = os.path.join(SURVEYS, world_id)
    if os.path.isdir(dest):
        if sha(os.path.join(dest, "manifest.json")) == sha(os.path.join(folder, "manifest.json")):
            return dest
        sys.exit(f"a different survey already sits at {dest}: a new run is a new world id")
    os.makedirs(SURVEYS, exist_ok=True)
    shutil.copytree(folder, dest)
    # (The files read-only, the folder not: the energy and surface packages are placed beside them.)
    for f in os.listdir(dest):
        os.chmod(os.path.join(dest, f), stat.S_IREAD | stat.S_IRGRP | stat.S_IROTH)
    os.chmod(dest, stat.S_IRWXU | stat.S_IRGRP | stat.S_IXGRP | stat.S_IROTH | stat.S_IXOTH)
    return dest


def packages(text, folder, dest, world_id):
    """The energy and surface packages beside the survey folder: energy copied and summarised, surface pointed at."""
    world = os.path.dirname(os.path.abspath(folder))
    text = re.sub(r"^(energy|bake):\n(?:  .*\n)*", "", text, flags=re.M)
    blocks = ""
    en = os.path.join(world, "energy")
    if os.path.isfile(os.path.join(en, "manifest.json")):
        em = json.load(open(os.path.join(en, "manifest.json"), encoding="utf-8"))
        if em.get("format") != "planet-sim-energy/1" or not em.get("immutable"):
            sys.exit("energy: not a finished planet-sim-energy/1 package")
        for name, want in em["files"].items():
            if sha(os.path.join(en, name)) != want["sha256"]:
                sys.exit(f"energy: {name} does not match its manifest")
        edest = os.path.join(dest, "energy")
        if not os.path.isdir(edest):
            shutil.copytree(en, edest)
            for f in os.listdir(edest):
                os.chmod(os.path.join(edest, f), stat.S_IREAD | stat.S_IRGRP | stat.S_IROTH)
        es = json.load(open(os.path.join(en, "summary.json"), encoding="utf-8"))
        blocks += ("energy:\n" + f'  format: "planet-sim-energy/1"\n' + f"  folder: {json.dumps(os.path.relpath(edest, ROOT).replace(os.sep, '/'))}\n"
                   + f"  manifest_sha256: {json.dumps(sha(os.path.join(edest, 'manifest.json')))}\n"
                   + "".join(f"  {k}: {es[k]}\n" for k in ("basins", "oil_fields", "gas_fields", "coalfields") if k in es)
                   + f"  oil_in_place: {float(f'{es['oil_in_place_mmbbl'] * 158987.0:.4g}')}\n"
                   + f"  gas_in_place: {float(f'{es['gas_in_place_bcf'] * 28.317e6:.4g}')}\n"
                   + f"  coal_in_place: {float(f'{es['coal_mt'] * 1e9:.4g}')}\n")
        print(f"energy: {es.get('oil_fields')} oil and {es.get('gas_fields')} gas fields, {es.get('coalfields')} coalfields, copied to {os.path.relpath(edest, ROOT)}")
    sf = os.path.join(world, "surface")
    if os.path.isfile(os.path.join(sf, "latest.json")):
        latest = json.load(open(os.path.join(sf, "latest.json"), encoding="utf-8"))
        vdir = os.path.join(sf, latest["path"]); vm_path = os.path.join(vdir, "manifest.json")
        if sha(vm_path) != latest["manifest_sha256"]:
            sys.exit("surface: latest.json does not match the version's manifest")
        vm = json.load(open(vm_path, encoding="utf-8"))
        sdest = os.path.join(dest, "surface"); os.makedirs(sdest, exist_ok=True)
        for name in ("latest.json",):
            shutil.copyfile(os.path.join(sf, name), os.path.join(sdest, name))
        shutil.copyfile(vm_path, os.path.join(sdest, "manifest.json"))
        blocks += ("bake:\n" + f'  format: "planet-sim-surface/1"\n' + f"  version: {latest['version']}\n"
                   + f"  path: {json.dumps(f'worlds/{world_id}/surface/{latest['path']}')}\n"
                   + f"  manifest_sha256: {json.dumps(latest['manifest_sha256'])}\n"
                   + (f"  files: {vm['files'] if isinstance(vm.get('files'), int) else len(vm.get('files', {}))}\n")
                   + (f"  bytes: {vm['bytes']}\n" if "bytes" in vm else "")
                   + (f"  note: {json.dumps(vm['note'])}\n" if vm.get("note") else ""))
        print(f"surface: v{latest['version']}, {vm.get('bytes', 0) / 1e9:.2f} GB in the worlds store; pointer and manifest kept")
    if not blocks:
        return text
    head, rest = text.split("\nidentity:\n", 1)
    return head + "\n" + blocks + "identity:\n" + rest


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    args = [a for a in argv[1:] if not a.startswith("--")]
    folder = args[0]
    if os.path.isdir(os.path.join(folder, "survey")):      # the world's folder: its survey is inside
        folder = os.path.join(folder, "survey")
    m, summary = check(folder)
    key = args[1] if len(args) > 1 else summary.get("body")
    if not key:
        sys.exit("the survey names no body (summary.body) and none was given")
    replace = "--replace" in argv
    if summary.get("body") not in (None, key):
        sys.exit(f"survey says it was seeded from {summary['body']}, not {key}")
    path = body_file(key)
    text = open(path, encoding="utf-8").read()
    rec = yaml.safe_load(text)
    radius = (rec.get("physical") or {}).get("radius")
    if radius and abs(summary["radius_m"] - radius) > 0.01 * radius:
        sys.exit(f"survey radius {summary['radius_m']:,.0f} m is not {key}'s {radius:,.0f} m: this world was not grown from this record")
    if rec.get("provenance") == "baked" and not replace and rec["survey"]["world_id"] != m["world_id"]:
        sys.exit(f"{key} is already baked from world {rec['survey']['world_id']}; --replace to point it at another world")
    dest = place(folder, m["world_id"])
    survey = {"format": FORMAT, "world_id": m["world_id"], "folder": os.path.relpath(dest, ROOT).replace(os.sep, "/"),
              "manifest_sha256": sha(os.path.join(dest, "manifest.json")), "simulator_commit": m["simulator"]["commit"], "created": m["created"],
              "land_share": round(summary["land_pct"] / 100, 4), "deposits": summary["deposits"], "districts": summary["districts"], "belts": summary["belts"],
              "contained": [{"commodity": k, "mass": float(f"{v['amount'] * {'t': 1000.0, 'ct': 0.0002}[v['unit']]:.4g}")} for k, v in summary["contained"].items()]}
    # The record: provenance, the survey block, and the seed's guesses for derived figures dropped.
    text = re.sub(r"^provenance: .*\n", "provenance: baked\n", text, count=1, flags=re.M)
    text = re.sub(r"^survey:\n(?:  .*\n)*", "", text, flags=re.M)
    text = re.sub(r"^# (Curated|Baked): .*\n", "", text, flags=re.M)
    head, rest = text.split("\nprovenance: baked\n", 1)
    note = f"# Baked: grown by the planet simulation (world {m['world_id']}, simulator {m['simulator']['commit'][:12]}). The survey it points at owns what the run derived; this record owns the inputs."
    block = "survey:\n" + "".join(f"  {k}: {json.dumps(v)}\n" for k, v in survey.items() if k != "contained") + "  contained:\n" + "".join(f"    - {json.dumps(c)}\n" for c in survey["contained"])
    text = head + "\n" + note + "\nprovenance: baked\n" + block + rest
    yaml.safe_load(text)
    open(path, "w", encoding="utf-8").write(text)
    text = packages(text, folder, dest, m["world_id"])
    yaml.safe_load(text)
    open(path, "w", encoding="utf-8").write(text)
    print(f"{key}: baked from {m['world_id']} at {survey['folder']} ({summary['deposits']} deposits, land {summary['land_pct']}%)")
    print("now: python3 tools/standards/build.py")


if __name__ == "__main__":
    main(sys.argv)
