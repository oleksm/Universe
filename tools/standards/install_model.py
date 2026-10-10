"""The asset installer: a model (.glb) to a registry record and the game, in one command.

    python3 tools/standards/install_model.py <model.glb> --as=<key>                     # dry run: checks, prints what it would do
    python3 tools/standards/install_model.py <model.glb> --as=<key> --apply             # write the package and the record, validate, build
    python3 tools/standards/install_model.py <model.glb> --as=<key> --push              # and commit and push on fso

Options: --source=<file.blend> (the source it was exported from: its path and sha256 go in the manifest), --note=<text>,
--about=<file.yaml|json> (the modeller's account, required with --push: sections model, work, considerations, and
optionally stats and evidence; it goes in the manifest as `about`, beside the stats the installer measures itself).

It reads the model itself (no manifest from the modeller needed): its size (the world bounds of its meshes), its node
names, its triangles (meshes named COL_* are collision and not counted), and checks them against the record: the three
sides, sorted, each 85% to 110% of the record's (the registry is the size authority: a mismatch is a fix to the model or
a request to the registry, never a silent edit), the nodes the kind requires (equipment: mount*, and nozzle*, duct*,
contact*, door* or dock* by what it does), the triangle budget. Then it writes the package in the assets store
(UNIVERSE_ASSETS, else ~/git/freefall-assets): models/<key tail>/v<N>/ with model.glb and manifest.json (format
freefall-model/1, key, kind, version, size checked, nodes, triangles, source, date, files with sha256 and size), points
the record's `visual` at it, runs the validators and the registry build (puts the record back if either fails), and with
--push commits on fso and pushes. The same file again changes nothing. Docs: docs/asset-contract.md.
"""
import datetime, json, os, re, shutil, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import FORMAT, ROOT, check_model, inspect_glb, read_about, records, sha, size_of, store, tail  # noqa: E402


def die(msg):
    sys.exit(f"install_model: {msg}")


def git(*a):
    return subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True, check=True).stdout.strip()


def main(argv):
    if len(argv) < 2 or argv[1].startswith("-"):
        sys.exit(__doc__)
    glb = os.path.abspath(os.path.expanduser(argv[1]))
    opt = {a.split("=", 1)[0][2:]: (a.split("=", 1)[1] if "=" in a else True) for a in argv[2:]}
    key = opt.get("as") or die("--as=<registry key>")
    apply, push = bool(opt.get("apply") or opt.get("push")), bool(opt.get("push"))
    if not os.path.isfile(glb):
        die(f"no file {glb}")
    hit = [(k, kk, p, r) for k, kk, p, r in records() if kk == key]
    if not hit:
        die(f"no record {key} of a kind that takes a model (equipment, module, building, structure, gate, hull)")
    kind, _, rec_path, rec = hit[0]
    try:
        info = inspect_glb(glb)
    except Exception as e:
        die(f"{glb}: {e}")
    print(f"install_model: {os.path.basename(glb)} as {key} ({kind}){' [dry run]' if not apply else ''}")
    print(f"  model: {info['bounds'][0]:.3g} x {info['bounds'][1]:.3g} x {info['bounds'][2]:.3g} m, {info['triangles']:,} triangles, {len(info['nodes'])} nodes")
    size = size_of(kind, rec)
    print(f"  record: {' x '.join(f'{v:.3g}' for v in size) + ' m' if size else 'no size'}")
    bad = check_model(kind, rec, info)
    about = None
    if isinstance(opt.get("about"), str):
        more, about = read_about(os.path.expanduser(opt["about"]))
        bad += more
    elif push:
        bad.append("--push needs --about=<file> (model, work, considerations; stats, evidence): the package says what it is (docs/asset-contract.md)")
    if bad:
        die("not installed:\n  - " + "\n  - ".join(bad))
    print("  1. checks: size, nodes and budget fit")

    # 2. the package
    base = os.path.join(store(), "models", tail(key))
    gsha = sha(glb)
    versions = sorted(int(d[1:]) for d in os.listdir(base) if re.fullmatch(r"v\d+", d)) if os.path.isdir(base) else []
    def installed(v):     # (the same model with the same account: that version again)
        d = os.path.join(base, f"v{v}")
        try:
            return sha(os.path.join(d, "model.glb")) == gsha and json.load(open(os.path.join(d, "manifest.json"))).get("about") == about
        except (OSError, ValueError):
            return False
    same = next((v for v in versions if installed(v)), None)
    version = same or (max(versions) + 1 if versions else 1)
    rel = f"models/{tail(key)}/v{version}"
    dest = os.path.join(store(), rel)
    print(f"  2. package {rel}: {'unchanged' if same else 'new'}")
    cur = rec.get("visual") or {}
    if same and cur.get("path") == rel:
        print("  nothing to change: the record already has this model.")
        return
    if not apply:
        print(f"  3. record {os.path.relpath(rec_path, ROOT)}: visual {cur.get('path')} -> {rel}")
        print("  dry run: nothing written. --apply to do it, --push to commit and push too.")
        return
    if not same:
        os.makedirs(dest, exist_ok=True)
        shutil.copyfile(glb, os.path.join(dest, "model.glb"))
        src = opt.get("source") if isinstance(opt.get("source"), str) else None
        repo = subprocess.run(["git", "-C", os.path.dirname(os.path.abspath(src)), "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip() if src and os.path.exists(src) else None
        manifest = {"format": FORMAT, "key": key, "kind": kind, "version": version, "file": "model.glb",
                    "bounds_m": [info["lo"], info["hi"]], "size_m": info["bounds"], "record_size_m": size, "nodes": info["nodes"], "triangles": info["triangles"],
                    "measured": info["stats"], **({"about": about} if about else {}),
                    "source": {"file": src and os.path.abspath(src), "sha256": sha(src) if src and os.path.exists(src) else None, "commit": repo, "exported": os.path.basename(glb)},
                    "date": datetime.date.today().isoformat(), **({"note": opt["note"]} if isinstance(opt.get("note"), str) else {}),
                    "files": {"model.glb": {"sha256": gsha, "size": os.path.getsize(glb)}}}
        json.dump(manifest, open(os.path.join(dest, "manifest.json"), "w"), indent=1)
    msha = sha(os.path.join(dest, "manifest.json"))

    # 3. the record
    text = open(rec_path, encoding="utf-8").read()
    block = f'visual:\n  format: "{FORMAT}"\n  path: "{rel}"\n  manifest_sha256: "{msha}"\n  version: {version}\n  triangles: {info["triangles"]}\n'
    if re.search(r"^visual:\n", text, flags=re.M):
        new = re.sub(r"^visual:\n(?:  .*\n)+", block, text, count=1, flags=re.M)
    elif re.search(r"^life:", text, flags=re.M):
        new = re.sub(r"^(life:)", block.replace("\\", "\\\\") + r"\1", text, count=1, flags=re.M)
    elif re.search(r"^basis:", text, flags=re.M):
        new = re.sub(r"^(basis:)", block.replace("\\", "\\\\") + r"\1", text, count=1, flags=re.M)
    else:
        new = text.rstrip("\n") + "\n" + block
    yaml.safe_load(new); open(rec_path, "w", encoding="utf-8").write(new)
    print(f"  3. record {os.path.relpath(rec_path, ROOT)}: visual -> {rel}")
    for cmd in (["tools/standards/validate.py"], ["tools/standards/build.py"]):
        out = subprocess.run([sys.executable, *cmd], capture_output=True, text=True, cwd=ROOT)
        if out.returncode:
            open(rec_path, "w", encoding="utf-8").write(text)
            die(f"{cmd[0]} failed, the record is put back:\n{(out.stdout + out.stderr)[-1500:]}")
    print("  4. validators and registry build: pass")
    if push:
        if git("branch", "--show-current") != "fso":
            die("--push commits on fso only")
        git("add", "--", os.path.relpath(rec_path, ROOT), "standards/index.html", "standards/changes.yaml")
        git("commit", "-q", "-m", f"{key}: model v{version} installed ({info['triangles']:,} triangles)\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>")
        git("push", "-q", "origin", "fso")
        print(f"  5. committed and pushed: {git('log', '--oneline', '-1')}")
    else:
        print("  5. not committed (--push to commit and push on fso)")


if __name__ == "__main__":
    main(sys.argv)
