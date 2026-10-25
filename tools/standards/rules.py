"""Say it once: a basis text repeated in many records becomes a rule (standards/SFO/metadata/rules/), and those
records' basis entries name it (`rule: rule.<slug>`) instead of repeating it.

    python3 tools/standards/rules.py            # rules for every text in 10+ records, records rewritten
    python3 tools/standards/rules.py --check    # list what would become a rule; exit 1 if any

Generated records follow through lib.ruled in their generators. Notes the build reads by their words (stand-ins,
fitted-within) stay as written.
"""
import glob, os, re, sys
from collections import Counter, defaultdict
import yaml

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

MIN = 10
KEEP = ("Not worked out", "Not measured", "Fitted within")
RULES = os.path.join(lib.TREE, "SFO", "metadata", "rules")
STOP = set("a an the of its it is in on to and or as by for from with at be this that not no".split())


def sig(e):
    if not isinstance(e, dict) or "rule" in e or "tier" not in e or not set(e) <= {"of", "tier", "source", "note", "review"}:
        return None
    if len(str(e.get("note") or "") + str(e.get("source") or "")) < 60 or str(e.get("note") or "").startswith(KEEP):
        return None
    return (e.get("tier"), e.get("source"), e.get("note"), bool(e.get("review")))


def entries(n):
    if isinstance(n, dict):
        for k, v in n.items():
            if k == "basis" and isinstance(v, list):
                yield from v
            else:
                yield from entries(v)
    elif isinstance(n, list):
        for x in n:
            yield from entries(x)


def records():
    for f in sorted(glob.glob(os.path.join(lib.TREE, "**", "*.yaml"), recursive=True)):
        if "/schema/" not in f and not f.startswith(RULES) and f.count(os.sep) - lib.TREE.count(os.sep) >= 3:
            yield f


def slug(text, taken):
    words = [w for w in re.findall(r"[a-z0-9]+", text.lower()) if w not in STOP][:5]
    s = "-".join(words) or "rule"
    k, i = s, 2
    while "rule." + k in taken:
        k, i = f"{s}-{i}", i + 1
    return k


def make(found):
    taken = set(lib.rules().values())
    for (tier, source, note, review), n in found:
        text = note or source
        k = slug(text, taken)
        taken.add("rule." + k)
        about = re.split(r"(?<=[a-z0-9)])[.;:] ", text, maxsplit=1)[0][:120]
        d = {"identity": {"key": "rule." + k, "name": about[:60], "about": about}, "statement": text, "tier": tier}
        if source:
            d["source"] = source
        if review:
            d["review"] = True
        open(os.path.join(RULES, k + ".yaml"), "w", encoding="utf-8").write(
            "# yaml-language-server: $schema=../../schema/rule.schema.yaml\n" + yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=120))
    lib._rules = None


def dumped(e, ind, width):
    return "".join(ind + l + "\n" for l in yaml.safe_dump([e], sort_keys=False, allow_unicode=True, width=width).splitlines())


def rewrite(f):
    s = open(f, encoding="utf-8").read()
    d = yaml.safe_load(s)
    want = lib.ruled(d)
    if want == d:
        return 0
    t, n = s, 0
    for old, new in zip(entries(d), entries(want)):
        if old == new:
            continue
        for width in (120, 1000, 230, 150, 80):
            for ind in ("", "  ", "    ", "      "):
                a = dumped(old, ind, width)
                if a in t:
                    t = t.replace(a, dumped(new, ind, width), 1)
                    n += 1
                    break
            else:
                continue
            break
    if yaml.safe_load(t) != want:
        lines = t.split("\n")
        for i, line in enumerate(lines):
            m = re.match(r"^(\s*)- (\{.*\})\s*$", line)
            if m:
                try:
                    e = yaml.safe_load(m.group(2))
                except yaml.YAMLError:
                    continue
                r_ = lib.ruled({"basis": [e]})["basis"][0]
                if r_ != e:
                    lines[i] = f"{m.group(1)}- {{ of: [{', '.join(r_['of'])}], rule: {r_['rule']} }}"
        t = "\n".join(lines)
    if yaml.safe_load(t) != want:
        print("left as it is:", os.path.relpath(f, lib.ROOT))
        return 0
    open(f, "w", encoding="utf-8").write(t)
    return n or 1


def main(check):
    seen, owned = Counter(), defaultdict(set)
    for f in records():
        try:
            d = yaml.safe_load(open(f, encoding="utf-8"))
        except yaml.YAMLError:
            continue
        for e in entries(d):
            k = sig(e)
            if k:
                seen[k] += 1
    found = [(k, n) for k, n in seen.most_common() if n >= MIN and k not in lib.rules()]
    if check:
        for (tier, source, note, review), n in found:
            print(f"  {n:4} {(note or source)[:100]}")
        print(f"{len(found)} basis text(s) in {MIN}+ records not yet a rule")
        sys.exit(1 if found else 0)
    make(found)
    total = sum(rewrite(f) for f in records() if not (lib.owned_by(f) and not lib.owned_by(f).get("keys")))
    print(f"{len(found)} new rules; {total} records rewritten (generated ones follow when their generator runs)")


if __name__ == "__main__":
    main("--check" in sys.argv)
