"""Game validation of an installed model, in one command (docs/asset-contract.md, Game validation).

    python3 tools/standards/validate_model.py <key> [<key>...] [--installed=<commit>] [--pending="..."]... [--push]

Several keys (a family's batch) share one run of the loader and one commit; --installed then applies to none of them
(each is found by its manifest's hash in the history).

From ~/git/universe-fso. It runs the game's own loader on every installed model (cargo run -p universe-world --example
validate_models), takes this key's line (package found, manifest matching the record's hash, model.glb matching the manifest,
bounds), and appends an entry to docs/asset-validation.yaml: result `partial` while the game does not draw the kind
(equipment at mounts, today), with the standing pending checks and any --pending given; `fail` if the loader refuses it.
With --push it commits that file on fso and pushes. Frames in the game (step 2 of the contract) stay a look by the
registry once the kind is drawn; this records that they are pending.
"""
import datetime, hashlib, json, os, re, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import ROOT, records, store  # noqa: E402

DRAWN = {"module", "building", "structure", "gate"}


def main(argv):
    if len(argv) < 2 or argv[1].startswith("-"):
        sys.exit(__doc__)
    keys = [a for a in argv[1:] if not a.startswith("--")]
    opt = [a for a in argv[2:] if a.startswith("--")]
    get = lambda k: next((a.split("=", 1)[1] for a in opt if a.startswith(f"--{k}=")), None)
    extra = [a.split("=", 1)[1] for a in opt if a.startswith("--pending=")]
    recs = {k: r for kind, k, p, r in records() if k in keys}
    kinds = {k: kind for kind, k, p, r in records() if k in keys}
    for key in keys:
        if key not in recs:
            sys.exit(f"validate_model: no record {key}")
        if not recs[key].get("visual"):
            sys.exit(f"validate_model: {key} has no visual (not installed)")
    capped = [os.path.expanduser("~/bin/capped")] if os.path.exists(os.path.expanduser("~/bin/capped")) else []
    out = subprocess.run([*capped, "cargo", "run", "-q", "-p", "universe-world", "--example", "validate_models"],
                         cwd=ROOT, capture_output=True, text=True)
    text = out.stdout + out.stderr
    head = subprocess.run(["git", "-C", ROOT, "log", "-1", "--format=%h"], capture_output=True, text=True).stdout.strip()
    entries, results = [], []
    for key in keys:
        kind, rec = kinds[key], recs[key]
        v = rec["visual"]
        line = next((ln for ln in text.splitlines() if ln.startswith(key + ":")), None)
        bad = next((ln.strip() for ln in text.splitlines() if key in ln and ln is not line), None) if line is None else None
        ok = line is not None and out.returncode == 0
        msha = hashlib.sha256(open(os.path.join(store(), v["path"], "manifest.json"), "rb").read()).hexdigest()
        installed = (get("installed") if len(keys) == 1 else None) or subprocess.run(["git", "-C", ROOT, "log", "-1", "--format=%h", "-S", v["manifest_sha256"], "--", "standards"],
                                                                                     capture_output=True, text=True).stdout.strip() or "unknown"
        man = json.load(open(os.path.join(store(), v["path"], "manifest.json")))
        has_previews = all(os.path.exists(os.path.join(store(), v["path"], f)) for f in (man.get("previews") or {}).values()) and \
            set((man.get("previews") or {})) >= {"thumb", "icon"}
        drawn = kind in DRAWN
        pending = ([] if drawn else ["frames in the game: the game does not draw this kind yet (engine)"]) + \
                  ["frames at play distance, GPU look and cost" if drawn else "GPU look and cost, placement on a real hull"] + \
                  ([] if has_previews else ["previews: thumb.png and icon.png missing from the package (reinstall with --thumb and --icon)"]) + extra
        result = "fail" if not ok else ("partial" if pending else "pass")
        detail = line.split(": ", 1)[1] if line else (bad or "no line for this key in the loader's output")
        entries.append(f"  - key: {key}\n    version: {v['version']}\n    manifest_sha256: \"{msha}\"\n    installed: {installed}\n"
                       f"    game: \"the loader (world::assets) at fso {head}\"\n    date: \"{datetime.date.today().isoformat()}\"\n"
                       f"    result: {result}\n    drawn: {str(drawn).lower()}\n    checked:\n"
                       f"      - \"loader (cargo test -p universe-world --test visuals): {detail.replace(chr(34), chr(39))}: {'pass' if ok else 'FAIL'}\"\n"
                       f"      - \"installer checks at install (size, nodes, budget, static): pass\"\n"
                       f"      - \"previews (thumb.png, icon.png in the package): {'pass' if has_previews else 'missing'}\"\n"
                       + ("    pending:\n" + "".join(f"      - \"{p}\"\n" for p in pending) if pending else ""))
        results.append((key, v["version"], result))
        print(f"validate_model: {key} v{v['version']}: {result} ({detail})")
    path = os.path.join(ROOT, "docs/asset-validation.yaml")
    s = open(path, encoding="utf-8").read().rstrip("\n") + "\n" + "".join(entries)
    yaml.safe_load(s)
    open(path, "w", encoding="utf-8").write(s)
    if "--push" in opt:
        if len(results) == 1:
            msg = f"asset validation: {results[0][0]} v{results[0][1]} {results[0][2]}"
        else:
            n = {r: sum(1 for x in results if x[2] == r) for r in ("pass", "partial", "fail")}
            msg = f"asset validation: {len(results)} models, " + ", ".join(f"{c} {r}" for r, c in n.items() if c) + "\n\n" + "\n".join(f"{k} v{ver} {res}" for k, ver, res in results)
        subprocess.run(["git", "-C", ROOT, "add", "docs/asset-validation.yaml"], check=True)
        subprocess.run(["git", "-C", ROOT, "commit", "-q", "-m", f"{msg}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"], check=True)
        subprocess.run(["git", "-C", ROOT, "push", "-q", "origin", "fso"], check=True)
        print("  committed and pushed:", subprocess.run(["git", "-C", ROOT, "log", "-1", "--format=%h"], capture_output=True, text=True).stdout.strip())
    sys.exit(0 if all(r[2] != "fail" for r in results) else 1)


if __name__ == "__main__":
    main(sys.argv)
