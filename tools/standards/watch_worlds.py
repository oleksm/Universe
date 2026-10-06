"""Watch the planet simulation's worlds store and install what is new, for review (docs/survey-contract.md).

    python3 tools/standards/watch_worlds.py --once           # check now, install what is new, stage it, print the review
    python3 tools/standards/watch_worlds.py                  # keep watching (every 5 minutes), the same each time
    python3 tools/standards/watch_worlds.py --once --commit  # and commit on the current branch (never pushed)

What it does when the store's releases.json has changed since the last install: runs world_install.py --store, the two
validators and build.py; appends the release to docs/worlds-releases.md (what came in, from which world, with hashes);
stages everything under standards/ and the docs; prints the Worlds table and `git diff --cached --stat`; sends a desktop
notice if notify-send is there. Nothing is committed unless --commit, and nothing is ever pushed: the review is yours.
The last installed index is remembered in standards/Celestial/surveys/.installed.json. Run from the repository root.
"""
import json, os, shutil, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
STORE = os.environ.get("UNIVERSE_WORLDS") or os.path.expanduser("~/git/planet-sim/out/worlds")
STATE = os.path.join(ROOT, "standards", "Celestial", "surveys", ".installed.json")
LOG = os.path.join(ROOT, "docs", "worlds-releases.md")
EVERY = 300


def run(*cmd, check=True):
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if check and r.returncode != 0:
        print(r.stdout[-2000:], r.stderr[-2000:])
        sys.exit(f"{' '.join(cmd)} failed")
    return r.stdout


def index():
    p = os.path.join(STORE, "releases.json")
    return json.load(open(p, encoding="utf-8")) if os.path.isfile(p) else None


def fingerprint(rel):
    """What matters of the index: each current world with a body, and its packages' hashes."""
    return {w["world_id"]: {k: (v or {}).get("manifest_sha256") for k, v in (w.get("packages") or {}).items()} for w in rel["worlds"] if w.get("status") == "current" and w.get("body")}


def install(rel, before, commit):
    now = fingerprint(rel)
    new = {w: p for w, p in now.items() if before.get(w) != p}
    print(f"{len(new)} world(s) new or changed: {', '.join(new) or '-'}")
    table = run(sys.executable, os.path.join("tools", "standards", "world_install.py"), "--store", STORE)
    v = run(sys.executable, os.path.join("tools", "standards", "validate.py")); print(v.strip().splitlines()[-1])
    run("uv", "run", "-q", "--no-project", "--with", "jsonschema", "--with", "pyyaml", "python3", os.path.join("tools", "standards", "check.py"))
    b = run(sys.executable, os.path.join("tools", "standards", "build.py")); print(b.strip().splitlines()[0][:80])
    # the release log, for the review
    lines = [f"\n## {time.strftime('%Y-%m-%d %H:%M')} — store index of {rel.get('updated', '?')}\n"]
    for wid in sorted(new):
        w = next(x for x in rel["worlds"] if x["world_id"] == wid)
        pk = ", ".join(f"{k} {v['manifest_sha256'][:12]}" + (f" v{v['version']}" if v.get("version") else "") for k, v in (w.get("packages") or {}).items() if v) or "no packages"
        lines.append(f"- **{wid}** → `{w['body']}`: {pk}; released {w.get('released', '?')}\n")
    if not new:
        lines.append("- nothing new; the store's index changed in what is superseded or has no body\n")
    if not os.path.isfile(LOG):
        lines.insert(0, "# Worlds installed from the planet simulation's store\n\nAppended by tools/standards/watch_worlds.py on every install, for review: which world came in for which body, with its packages' manifest hashes.\n")
    open(LOG, "a", encoding="utf-8").write("".join(lines))
    json.dump({"updated": rel.get("updated"), "worlds": now}, open(STATE, "w", encoding="utf-8"), indent=1)
    run("git", "add", "standards", "docs", "content")
    stat = run("git", "diff", "--cached", "--stat")
    print(table.strip().splitlines()[-min(6, len(table.strip().splitlines())):] and "\n".join(table.strip().splitlines()[-6:]))
    print(stat.strip()[-1500:] or "nothing to stage")
    if commit and stat.strip():
        msg = "Worlds installed from the store: " + ", ".join(f"{w} for {next(x for x in rel['worlds'] if x['world_id'] == w)['body'].split('.')[-1]}" for w in sorted(new))
        run("git", "commit", "-q", "-m", msg + "\n\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>")
        print("committed (not pushed):", msg)
    if shutil.which("notify-send"):
        subprocess.run(["notify-send", "Freefall Facts", f"Worlds installed: {', '.join(new) or 'nothing new'} — review git diff --cached"], check=False)


def main(argv):
    once, commit = "--once" in argv, "--commit" in argv
    while True:
        rel = index()
        if rel is None:
            print(f"no releases.json at {STORE}")
        else:
            before = json.load(open(STATE, encoding="utf-8")) if os.path.isfile(STATE) else {}
            if rel.get("updated") != before.get("updated") or fingerprint(rel) != before.get("worlds", {}):
                install(rel, before.get("worlds", {}), commit)
            else:
                print(f"{time.strftime('%H:%M')}: nothing new in the store (index of {rel.get('updated')})")
        if once:
            break
        time.sleep(EVERY)


if __name__ == "__main__":
    main(sys.argv)
