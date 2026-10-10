"""Install a grown planet as a body's world: from a planet directory to the game, in one command.

    python3 tools/standards/install_world.py <planet-dir> --world=TRE3 --as=Hoar                  # dry run: what it would do
    python3 tools/standards/install_world.py <planet-dir> --world=TRE3 --as=Hoar --apply          # do it, validate, build
    python3 tools/standards/install_world.py <planet-dir> --world=TRE3 --as=Hoar --push           # and commit and push on fso

Options:
    --survey=TRE3           the world whose survey (and energy, if it has one) the body takes; default the --world's, if
                            the worlds store has worlds/<id>/survey
    --snapshot-name=NAME    the ground snapshot's folder; default <planet>-<the root's sha256, 8 hex>, so the same file
                            always lands in the same place
    --root=FILE             the planet's root file in the planet dir; default ck_22_rivers_anchors.lines. Depth files
                            beside it named L<level>_<i>_<j>.lines are copied too
    --engine=PATH           the engine's lines reader (unfold-core/src/lines.rs); default the one the game's Cargo.toml
                            names, else ~/git/planet-unfold-engine/crates/unfold-core/src/lines.rs
    --climate               take the survey's climate onto the record (by default the record's climate is untouched)
    --note=TEXT             the snapshot's note

What it does, in order, and refuses cleanly at the first thing wrong:
  1. reads every line file's format and line kinds and checks them against the engine's reader (its PTL version and its
     Kind enum): a kind the installed engine does not read stops the install, so the game never breaks on it;
  2. makes the snapshot in the worlds store (UNIVERSE_WORLDS, else ~/git/planet-sim/out): worlds/<world>/ground/<name>/
     with the root as L0_0_0.lines, the depth files beside it, and manifest.json (format planet-unfold-tiles/1, world_id,
     body, name, radius_m from the file, l0, start, source {repo commit, file, date}, note, files with sha256 and size);
  3. points the body's record at it: survey and energy from the --survey world (tools/standards/world_install.py), ground
     from the snapshot (tools/standards/set_ground.py); radius, gravity, climate and lore stay the record's unless told;
  4. runs the validators and the registry build, and puts the record back if either fails;
  5. with --push, commits the record and the survey folder on fso and pushes.
Idempotent: a second run with the same files changes nothing and says so. Run from the fso worktree's root.
"""
import datetime, glob, hashlib, json, os, re, shutil, struct, subprocess, sys
import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import set_ground  # noqa: E402

FORMAT = "planet-unfold-tiles/1"
CLIMATE = ("mean_temperature", "temperature_low", "temperature_high", "rain")


def die(msg):
    sys.exit(f"install_world: {msg}")


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def store():
    p = os.environ.get("UNIVERSE_WORLDS") or os.path.expanduser("~/git/planet-sim/out")
    return p if os.path.isdir(p) else die("no worlds store (set UNIVERSE_WORLDS)")


# ---- the line files and the engine's reader

def read_lines(path):
    """A PTL2 file's header and the set of line kinds it carries, read without keeping the vertices."""
    b = open(path, "rb").read()
    if b[:4] != b"PTL2":
        die(f"{path}: not a PTL2 line file (magic {b[:4]!r})")
    version, body, radius, datum, level, i, j, lon0, lat0, deg = struct.unpack_from("<HIdfBIIddd", b, 4)
    at = 55
    (n,) = struct.unpack_from("<I", b, at); at += 4
    kinds = {}
    for k in range(n):
        kind = b[at]; at += 1 + 8 + 2 + 2 + 4 + 40
        (nv,) = struct.unpack_from("<I", b, at); at += 4 + nv * 28
        kinds[kind] = kinds.get(kind, 0) + 1
    if at != len(b):
        die(f"{path}: {len(b) - at} bytes after the last line")
    return {"version": version, "radius": radius, "level": level, "lines": n, "kinds": kinds}


def engine_reader(path):
    if not path:
        cargo = os.path.join(ROOT, "crates", "game", "Cargo.toml")
        m = re.search(r'unfold-view\s*=\s*\{\s*path\s*=\s*"([^"]+)"', open(cargo).read()) if os.path.exists(cargo) else None
        path = os.path.normpath(os.path.join(os.path.dirname(cargo), m.group(1), "..", "unfold-core", "src", "lines.rs")) if m else os.path.expanduser("~/git/planet-unfold-engine/crates/unfold-core/src/lines.rs")
    if not os.path.exists(path):
        die(f"no engine reader at {path} (give --engine)")
    s = open(path).read()
    version = int(re.search(r"pub const VERSION: u16 = (\d+);", s).group(1))
    enum = re.search(r"pub enum Kind \{(.*?)\n\}", s, flags=re.S).group(1)
    kinds = {int(v): name for name, v in re.findall(r"^\s*(\w+)\s*=\s*(\d+),", enum, flags=re.M)}
    return path, version, kinds


# ---- the record

def body_file(name):
    for f in glob.glob(os.path.join(ROOT, "standards/Celestial/metadata/systems/*/bodies/*.yaml")):
        i = (yaml.safe_load(open(f)) or {}).get("identity", {})
        if name in (i.get("key"), i.get("name")) or str(i.get("name", "")).split(" ")[0] == name:
            return f, i["key"]
    die(f"no body named {name}")


def climate_of(text):
    s = (yaml.safe_load(text) or {}).get("surface") or {}
    return {k: s[k] for k in CLIMATE if k in s}


def restore_climate(path, before):
    s = open(path).read()
    for k, v in before.items():
        s = re.sub(rf"^(  {k}: ).*$", lambda m: m.group(1) + str(v), s, count=1, flags=re.M)
    yaml.safe_load(s); open(path, "w").write(s)


def git(*a, check=True):
    return subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True, check=check).stdout.strip()


def main(argv):
    if len(argv) < 2 or argv[1].startswith("-"):
        sys.exit(__doc__)
    planet = os.path.abspath(os.path.expanduser(argv[1]))
    opt = {a.split("=", 1)[0][2:]: (a.split("=", 1)[1] if "=" in a else True) for a in argv[2:]}
    world, who = opt.get("world") or die("--world=<id>"), opt.get("as") or die("--as=<body>")
    apply, push = bool(opt.get("apply") or opt.get("push")), bool(opt.get("push"))
    root = os.path.join(planet, opt.get("root", "ck_22_rivers_anchors.lines"))
    if not os.path.isfile(root):
        die(f"no root file {root}")
    depth = sorted(glob.glob(os.path.join(planet, "L[1-9]_*.lines")))
    name = os.path.basename(planet.rstrip("/"))
    root_sha = sha(root)
    snap = opt.get("snapshot-name") or f"{name}-{root_sha[:8]}"
    rec_path, key = body_file(who)
    survey_world = opt.get("survey") or (world if os.path.isdir(os.path.join(store(), "worlds", world, "survey")) else None)
    print(f"install_world: {name} as {key} (world {world}){' [dry run]' if not apply else ''}")

    # 1. the kinds the files carry, against the engine's reader
    epath, ever, ekinds = engine_reader(opt.get("engine"))
    infos = {os.path.basename(f): read_lines(f) for f in [root] + depth}
    for fname, info in infos.items():
        if info["version"] != ever:
            die(f"{fname} is PTL2 version {info['version']}; the engine reads {ever} ({epath}): the install waits")
        unknown = sorted(set(info["kinds"]) - set(ekinds))
        if unknown:
            die(f"{fname} carries line kind(s) {', '.join(map(str, unknown))} ({sum(info['kinds'][k] for k in unknown):,} lines) that the engine does not read "
                f"(it reads {', '.join(f'{v} {n}' for v, n in sorted(ekinds.items()))}; {epath}): the install waits until its reader lands")
    r = infos[os.path.basename(root)]
    print(f"  1. kinds: {', '.join(f'{ekinds[k]} {n:,}' for k, n in sorted(r['kinds'].items()))} ({r['lines']:,} lines): all read by the engine ({os.path.relpath(epath, os.path.expanduser('~'))})")

    # 2. the snapshot
    rel = f"worlds/{world}/ground/{snap}"
    dest = os.path.join(store(), rel)
    files = {"L0_0_0.lines": root, **{os.path.basename(f): f for f in depth}}
    entries = {n: {"sha256": root_sha if src == root else sha(src), "size": os.path.getsize(src)} for n, src in files.items()}
    repo = subprocess.run(["git", "-C", planet, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip() or None
    date = datetime.date.fromtimestamp(os.path.getmtime(root)).isoformat()
    old = json.load(open(os.path.join(dest, "manifest.json"))) if os.path.exists(os.path.join(dest, "manifest.json")) else None
    manifest = {"format": FORMAT, "world_id": world, "body": key, "name": name, "radius_m": round(r["radius"]), "l0": "L0_0_0.lines",
                "start": (old or {}).get("start", {"lon": -133.0, "lat": -33.0}),
                "source": {"commit": repo, "file": os.path.relpath(root, os.path.expanduser("~/git")), "snapshot": snap, "date": date, **({"note": opt["note"]} if isinstance(opt.get("note"), str) else {})},
                "files": entries}
    same = old is not None and old.get("files") == entries and all(os.path.isfile(os.path.join(dest, n)) and os.path.getsize(os.path.join(dest, n)) == e["size"] for n, e in entries.items())
    print(f"  2. snapshot {rel}: {'unchanged' if same else ('replaced' if old else 'new')}, {len(entries)} file(s), {sum(e['size'] for e in entries.values()) / 1e6:.1f} MB, radius {manifest['radius_m']:,} m")
    if apply and not same:
        os.makedirs(dest, exist_ok=True)
        for n, src in files.items():
            shutil.copyfile(src, os.path.join(dest, n))
        json.dump(manifest, open(os.path.join(dest, "manifest.json"), "w"), indent=1)

    # 3. the record
    text = open(rec_path).read(); rec = yaml.safe_load(text)
    cur_survey = (rec.get("survey") or {}).get("world_id"); cur_ground = (rec.get("ground") or {}).get("path")
    msha = sha(os.path.join(dest, "manifest.json")) if os.path.exists(os.path.join(dest, "manifest.json")) and (same or apply) else None
    record_same = cur_survey == (survey_world or cur_survey) and cur_ground == rel and (rec.get("ground") or {}).get("manifest_sha256") == msha
    print(f"  3. record {os.path.relpath(rec_path, ROOT)}: survey {cur_survey} -> {survey_world or cur_survey}; ground {cur_ground} -> {rel}; climate {'from the survey' if opt.get('climate') else 'untouched'}{'; unchanged' if record_same else ''}")
    if not apply:
        print("  dry run: nothing written. --apply to do it, --push to commit and push too.")
        return
    if record_same:
        print("  nothing to change: the body already has this world.")
        return
    before = climate_of(text)
    try:
        if survey_world and survey_world != cur_survey:
            out = subprocess.run([sys.executable, os.path.join(ROOT, "tools/standards/world_install.py"), os.path.join(store(), "worlds", survey_world), key, "--replace"], capture_output=True, text=True, cwd=ROOT)
            if out.returncode:
                raise RuntimeError(f"world_install: {out.stderr or out.stdout}")
        if not opt.get("climate"):
            restore_climate(rec_path, before)
        os.chdir(ROOT)
        set_ground.main(key, rel, opt["note"] if isinstance(opt.get("note"), str) else None)
        # 4. validators and the build
        for cmd in ([sys.executable, "tools/standards/validate.py"], [sys.executable, "tools/standards/build.py"]):
            out = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT)
            if out.returncode:
                raise RuntimeError(f"{cmd[1]}: {(out.stdout + out.stderr)[-1500:]}")
        print("  4. validators and registry build: pass")
    except Exception as e:
        open(rec_path, "w").write(text)
        if survey_world and survey_world != cur_survey:
            shutil.rmtree(os.path.join(ROOT, "standards", "Celestial", "surveys", survey_world), ignore_errors=True)
        die(f"{e}\nthe record is put back as it was")
    # 5. commit and push
    if push:
        if git("branch", "--show-current") != "fso":
            die("--push commits on fso only")
        paths = [os.path.relpath(rec_path, ROOT), "standards/index.html", "standards/changes.yaml"] + ([f"standards/Celestial/surveys/{survey_world}"] if survey_world else [])
        git("add", "--", *paths)
        git("commit", "-q", "-m", f"{key.split('.')[-1]}: world {world} installed from {name} ({snap}){', survey ' + survey_world if survey_world else ''}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>")
        git("push", "-q", "origin", "fso")
        print(f"  5. committed and pushed: {git('log', '--oneline', '-1')}")
    else:
        print("  5. not committed (--push to commit and push on fso)")


if __name__ == "__main__":
    main(sys.argv)
