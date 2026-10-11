"""The pre-acceptance check of a model candidate, in one command, by the modeller: no registry agent needed unless it fails.

    python3 tools/standards/precheck_model.py /abs/candidate/delivery.yaml          # check, print, write the report
    python3 tools/standards/precheck_model.py /abs/candidate/delivery.yaml --quiet  # only the verdict line

From the candidate's delivery record (keys: asset, export, motion, about, source, hashes; paths relative to it) it:

1. checks every file named in `hashes` against its sha256;
2. runs the installer's dry run (install_model.py, with --source, --about and --motion where given): size, nodes,
   budget, the account's sections, and the motion file against its schema, the model and the record;
3. when there is a motion file, runs the game's own CPU consumer (engine's crates/engine/examples/ch_s2_fixture.rs,
   built from this worktree): the real model loader with the part transforms, every reference pose and a 513-pose sweep;
4. writes the report beside the delivery record, `review/registry-precheck.md`, with the verdict, every output and
   what still stands before the model moves in play (docs/ships/ch-s2-v11-precheck.md has the gates).

Exit 0: pass. Exit 1: a check failed (the report says which); send the registry a task then, with the report. Nothing is
installed or committed. Docs: docs/asset-contract.md, Pre-acceptance check.
"""
import datetime, os, subprocess, sys
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from assets import ROOT, sha  # noqa: E402


def run(cmd, timeout=900):
    try:
        p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=timeout)
        return p.returncode, (p.stdout + p.stderr).strip()
    except subprocess.TimeoutExpired:
        return 124, f"timed out after {timeout} s"


def main(argv):
    if len(argv) < 2 or argv[1].startswith("-"):
        sys.exit(__doc__)
    dpath = os.path.abspath(argv[1]); base = os.path.dirname(dpath)
    quiet = "--quiet" in argv
    d = yaml.safe_load(open(dpath, encoding="utf-8")) or {}
    at = lambda k: os.path.join(base, d[k]) if d.get(k) else None
    key, glb, motion, about, source = d.get("asset"), at("export"), at("motion"), at("about"), at("source")
    steps, ok = [], True
    if not key or not glb:
        sys.exit("precheck_model: the delivery record needs `asset` and `export`")

    # 1. hashes
    bad = []
    for rel, want in (d.get("hashes") or {}).items():
        f = os.path.join(base, rel)
        got = sha(f) if os.path.exists(f) else "missing"
        if got != want:
            bad.append(f"{rel}: {got[:16]} against {want[:16]}")
    steps.append(("hashes", not bad, "\n".join(bad) or f"{len(d.get('hashes') or {})} files match"))
    ok &= not bad

    # 2. installer dry run
    cmd = [sys.executable, "tools/standards/install_model.py", glb, f"--as={key}"]
    cmd += [f"--source={source}"] if source else []
    cmd += [f"--about={about}"] if about else []
    cmd += [f"--motion={motion}"] if motion else []
    cmd += [f"--{n}={at(n)}" for n in ("thumb", "icon") if at(n)]
    env_store = os.path.join("/tmp", f"precheck-store-{os.getpid()}")      # (a dry run writes nothing; the store is never touched)
    code, out = run(["env", f"UNIVERSE_ASSETS={env_store}", *cmd])
    steps.append(("installer dry run", code == 0, out))
    ok &= code == 0

    # 3. the game's CPU consumer
    if motion:
        capped = [os.path.expanduser("~/bin/capped")] if os.path.exists(os.path.expanduser("~/bin/capped")) else []   # (heavy builds under the memory cap)
        code, out = run([*capped, "cargo", "run", "-q", "--release", "-p", "universe-engine", "--example", "ch_s2_fixture", "--", motion, glb])
        passed = code == 0 and "PASS" in out
        steps.append(("game CPU consumer (engine's ch_s2_fixture)", passed, out[-3000:]))
        ok &= passed

    git = run(["git", "log", "-1", "--format=%h"])[1]
    verdict = "PASS" if ok else "FAIL"
    lines = [f"# Registry pre-acceptance check: {key} {d.get('candidate', '')}", "",
             f"{verdict} on {datetime.datetime.now():%Y-%m-%d %H:%M}, registry worktree {git} (fso). Not an install.", ""]
    for name, passed, out in steps:
        lines += [f"## {name}: {'pass' if passed else 'FAIL'}", "", "```", out, "```", ""]
    lines += ["## Still before it moves in play", "",
              "Acceptance and open QC (owner, modeller); install with --about and --motion (modeller) and the registry's",
              "validation entry; the game reading motion.json, drawing equipment at its mount, a per-engine gimbal command,",
              "live part motion, GPU look and cost (engine; the registry looks); full-assembly clearance on the final hull.",
              "Detail: ~/git/universe-fso/docs/ships/ch-s2-v11-precheck.md.", ""]
    os.makedirs(os.path.join(base, "review"), exist_ok=True)
    rep = os.path.join(base, "review", "registry-precheck.md")
    open(rep, "w", encoding="utf-8").write("\n".join(lines))
    print(f"precheck_model: {verdict} ({', '.join(n + (' ok' if p else ' FAILED') for n, p, _ in steps)}); report {rep}")
    if not quiet:
        for name, passed, out in steps:
            if not passed:
                print(f"--- {name}\n{out[-1500:]}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main(sys.argv)
