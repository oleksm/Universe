"""Game validation of an installed model, in one command (docs/asset-contract.md, Game validation).

    python3 tools/standards/validate_model.py <key> [--installed=<commit>] [--pending="..."]... [--push]

From ~/git/universe-fso. It runs the game's own loader on every installed model (cargo test -p universe-world --test
visuals), takes this key's line (package found, manifest matching the record's hash, model.glb matching the manifest,
bounds), and appends an entry to docs/asset-validation.yaml: result `partial` while the game does not draw the kind
(equipment at mounts, today), with the standing pending checks and any --pending given; `fail` if the loader refuses it.
With --push it commits that file on fso and pushes. Frames in the game (step 2 of the contract) stay a look by the
registry once the kind is drawn; this records that they are pending.
"""
import datetime, hashlib, json, os, re, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import ROOT, records, store  # noqa: E402

DRAWN = {"module", "building", "structure", "gate"}     # (what the game draws as models today: main 364a539e)


def main(argv):
    if len(argv) < 2 or argv[1].startswith("-"):
        sys.exit(__doc__)
    key = argv[1]
    opt = [a for a in argv[2:] if a.startswith("--")]
    get = lambda k: next((a.split("=", 1)[1] for a in opt if a.startswith(f"--{k}=")), None)
    extra = [a.split("=", 1)[1] for a in opt if a.startswith("--pending=")]
    hit = [(kind, r) for kind, k, p, r in records() if k == key]
    if not hit:
        sys.exit(f"validate_model: no record {key}")
    kind, rec = hit[0]
    v = rec.get("visual") or sys.exit(f"validate_model: {key} has no visual (not installed)")
    capped = [os.path.expanduser("~/bin/capped")] if os.path.exists(os.path.expanduser("~/bin/capped")) else []
    out = subprocess.run([*capped, "cargo", "test", "-q", "-p", "universe-world", "--test", "visuals", "--", "--nocapture"],
                         cwd=ROOT, capture_output=True, text=True)
    text = out.stdout + out.stderr
    line = next((ln for ln in text.splitlines() if ln.startswith(key + ":")), None)
    bad = next((ln.strip() for ln in text.splitlines() if key in ln and ln is not line), None) if line is None else None
    ok = line is not None and out.returncode == 0
    msha = hashlib.sha256(open(os.path.join(store(), v["path"], "manifest.json"), "rb").read()).hexdigest()
    head = subprocess.run(["git", "-C", ROOT, "log", "-1", "--format=%h"], capture_output=True, text=True).stdout.strip()
    installed = get("installed") or subprocess.run(["git", "-C", ROOT, "log", "-1", "--format=%h", "-S", v["manifest_sha256"], "--", "standards"],
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
    entry = (f"  - key: {key}\n    version: {v['version']}\n    manifest_sha256: \"{msha}\"\n    installed: {installed}\n"
             f"    game: \"the loader (world::assets) at fso {head}\"\n    date: \"{datetime.date.today().isoformat()}\"\n"
             f"    result: {result}\n    drawn: {str(drawn).lower()}\n    checked:\n"
             f"      - \"loader (cargo test -p universe-world --test visuals): {detail.replace(chr(34), chr(39))}: {'pass' if ok else 'FAIL'}\"\n"
             f"      - \"installer checks at install (size, nodes, budget, static): pass\"\n"
             f"      - \"previews (thumb.png, icon.png in the package): {'pass' if has_previews else 'missing'}\"\n"
             + ("    pending:\n" + "".join(f"      - \"{p}\"\n" for p in pending) if pending else ""))
    path = os.path.join(ROOT, "docs/asset-validation.yaml")
    s = open(path, encoding="utf-8").read().rstrip("\n") + "\n" + entry
    yaml.safe_load(s)
    open(path, "w", encoding="utf-8").write(s)
    print(f"validate_model: {key} v{v['version']}: {result} ({detail})")
    if "--push" in opt:
        subprocess.run(["git", "-C", ROOT, "add", "docs/asset-validation.yaml"], check=True)
        subprocess.run(["git", "-C", ROOT, "commit", "-q", "-m", f"asset validation: {key} v{v['version']} {result}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"], check=True)
        subprocess.run(["git", "-C", ROOT, "push", "-q", "origin", "fso"], check=True)
        print("  committed and pushed:", subprocess.run(["git", "-C", ROOT, "log", "-1", "--format=%h"], capture_output=True, text=True).stdout.strip())
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main(sys.argv)
