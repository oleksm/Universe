#!/usr/bin/env python3
"""Every record of the registry against the schema its first line names. Nothing else.
Needs: pip install -r tools/standards/requirements.txt

    python3 tools/standards/check.py        # lists what does not fit; exits 1 if anything
"""
import functools, pathlib, re, sys, urllib.request
import yaml
from jsonschema import Draft7Validator
from referencing import Registry, Resource

TREE = pathlib.Path(__file__).resolve().parents[2] / "standards"
read = functools.cache(lambda uri: Resource.from_contents(yaml.safe_load(pathlib.Path(urllib.request.url2pathname(uri.removeprefix("file:"))).read_text(encoding="utf-8"))))
SCHEMAS = Registry(retrieve=read)
checker = functools.cache(lambda uri: Draft7Validator({"$ref": uri}, registry=SCHEMAS))       # (one for each schema, not each record)
GENERATED = {TREE / "changes.yaml", TREE / "game-keys.yaml"}


def misfits():
    for path in sorted(p for p in TREE.rglob("*.yaml") if "schema" not in p.name and "sources" not in p.parts and p not in GENERATED):
        text = path.read_text(encoding="utf-8")
        named = re.match(r"# yaml-language-server: \$schema=(\S+)", text)
        if not named:
            yield path, "names no schema on its first line"
            continue
        for e in sorted(checker((path.parent / named[1]).resolve().as_uri()).iter_errors(yaml.safe_load(text) or {}), key=lambda e: list(e.absolute_path)):
            yield path, ".".join(map(str, e.absolute_path)) + ": " + e.message[:200]


if __name__ == "__main__":
    found = list(misfits())
    for path, what in found:
        print(f"{path.relative_to(TREE.parent)}: {what}")
    print(f"{len(found)} thing(s) that do not fit their schema")
    sys.exit(1 if found else 0)
