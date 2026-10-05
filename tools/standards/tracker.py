#!/usr/bin/env python3
"""standards/changes.yaml: the registry's change tracker. One line for every record: its key (its
kind and its name), when it last changed, its revision's status, what changed, and the hash of its
file. Written by this script and by the build; never by hand.

A record whose line says `complete` may not change: validate.py (and so the build) fails if its
file's hash is no longer the line's. To change one, the user reopens it:
    python3 tools/standards/tracker.py reopen <key> "<why>"
which sets its line to `review`; the record's own revision is then changed with it.

    python3 tools/standards/tracker.py          # bring the tracker up to date
"""
import datetime, hashlib, os, subprocess, sys
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
TREE = os.path.join(ROOT, "standards")
PATH = os.path.join(TREE, "changes.yaml")
HEADER = """# GENERATED. NEVER CHANGE THIS FILE BY HAND.
# The registry's change tracker, written by tools/standards/tracker.py and by the build.
# One line a record: its key (kind.name), when it last changed (UTC), its revision's status,
# what changed, and the hash of its file. A record whose line says `complete` may not change:
# validation fails if it does. Only the user reopens one:
#   python3 tools/standards/tracker.py reopen <key> "<why>"
"""


def sha(full):
    with open(full, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()[:16]


def status_of(rec, sch):
    """A record's status: its own revision's, or its kind's default."""
    own = (rec.get("revision") or {}).get("status") if isinstance(rec.get("revision"), dict) else None
    return own or ((((sch.get("properties") or {}).get("revision") or {}).get("default") or {}).get("status")) or ""


def read():
    if not os.path.exists(PATH):
        return {}
    with open(PATH, encoding="utf-8") as f:
        return {e["record"]: e for e in (yaml.safe_load(f) or [])}


def check(seen):
    """What breaks the tracker's one rule: a record that was complete and is no longer what it was."""
    was, now = read(), {key: full for key, full, _, _ in seen}
    for key, e in sorted(was.items()):
        if e.get("status") != "complete":
            continue
        if key not in now:
            yield os.path.join(TREE, "changes.yaml"), f"{key} is complete and its record is gone"
        elif sha(now[key]) != e.get("sha"):
            yield now[key], f"{key} is complete (standards/changes.yaml) and has been changed: the user reopens it first"


def what_changed(full, rec):
    """The groups of a record that differ from its last committed self."""
    rel = os.path.relpath(full, ROOT)
    try:
        old = yaml.safe_load(subprocess.run(["git", "show", "HEAD:" + rel.replace(os.sep, "/")], cwd=ROOT, capture_output=True, text=True, check=True).stdout) or {}
    except Exception:
        return "changed"
    groups = [g for g in list(rec) + [g for g in old if g not in rec] if rec.get(g) != old.get(g)]
    return "changed: " + ", ".join(groups) if groups else "changed"


def write(seen):
    was, out = read(), []
    at = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    for key, full, rec, sch in sorted(seen, key=lambda s: s[0]):
        h, st, e = sha(full), status_of(rec, sch), was.get(key)
        if e is None:
            e = {"record": key, "at": at, "status": st, "summary": "first tracked", "sha": h}
        elif e.get("sha") != h:
            e = {"record": key, "at": at, "status": st, "summary": what_changed(full, rec), "sha": h}
        elif e.get("status") != st:
            e = {"record": key, "at": at, "status": st, "summary": f"status: {e.get('status') or 'none'} to {st or 'none'}", "sha": h}
        out.append(e)
    q = lambda s: '"' + str(s).replace("\\", "\\\\").replace('"', '\\"') + '"'
    text = HEADER + "".join(f"- {{ record: {e['record']}, at: {q(e['at'])}, status: {e['status'] or '\"\"'}, summary: {q(e['summary'])}, sha: {q(e['sha'])} }}\n" for e in out)
    if not os.path.exists(PATH) or open(PATH, encoding="utf-8").read() != text:
        with open(PATH, "w", encoding="utf-8") as f:
            f.write(text)
    return len(out)


if __name__ == "__main__":
    sys.path.insert(0, HERE)
    import validate as V
    if len(sys.argv) >= 3 and sys.argv[1] == "reopen":
        was = read()
        key, why = sys.argv[2], " ".join(sys.argv[3:]) or "no reason given"
        if key not in was:
            sys.exit(f"{key}: not in the tracker")
        was[key].update(status="review", summary="reopened: " + why, at=datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
        q = lambda s: '"' + str(s).replace("\\", "\\\\").replace('"', '\\"') + '"'
        with open(PATH, "w", encoding="utf-8") as f:
            f.write(HEADER + "".join(f"- {{ record: {e['record']}, at: {q(e['at'])}, status: {e['status'] or '\"\"'}, summary: {q(e['summary'])}, sha: {q(e['sha'])} }}\n" for _, e in sorted(was.items())))
        print(f"{key}: reopened")
        sys.exit(0)
    found, unheld = V.check_all()
    if found or unheld:
        sys.exit(f"{len(found) + len(unheld)} thing(s) do not fit: the tracker is not written until the registry is whole (python3 tools/standards/validate.py)")
    print(f"standards/changes.yaml: {write(V.SEEN)} records")
