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
def _load(path):
    return yaml.safe_load(pathlib.Path(path).read_text(encoding="utf-8"))


def _resolve(ref, here):
    file, _, pointer = ref.partition("#")
    path = pathlib.Path(here).parent.joinpath(file).resolve() if file else pathlib.Path(here)
    node = _load(path)
    for step in [s for s in pointer.split("/") if s]:
        node = node[step]
    return node, str(path)


def rebase(node, path):
    """A node taken from the schema file `path` and inlined elsewhere: its `$ref`s made absolute so they still resolve."""
    if isinstance(node, list):
        return [rebase(x, path) for x in node]
    if not isinstance(node, dict):
        return node
    out = {}
    for k, v in node.items():
        if k == "$ref" and isinstance(v, str):
            file, _, pointer = v.partition("#")
            target = pathlib.Path(path).parent.joinpath(file).resolve() if file else pathlib.Path(path).resolve()
            out[k] = target.as_uri() + "#" + pointer
        else:
            out[k] = rebase(v, path)
    return out


def flatten(node, here):
    """The registry's `allOf` is derivation: a shape's properties and required fields become the object's own (so
    `additionalProperties: false` admits them), as the generator and validate.py read it. Done on the loaded schema before
    jsonschema sees it; a shape that only tightens a nested property's `required` is merged into that property."""
    if isinstance(node, list):
        return [flatten(x, here) for x in node]
    if not isinstance(node, dict):
        return node
    node = {k: flatten(v, here) for k, v in node.items()}
    shapes = node.pop("allOf", None)
    if shapes and (node.get("type") == "object" or "properties" in node):
        props = dict(node.get("properties") or {}); required = list(node.get("required") or [])
        for shape in shapes:
            path = here
            if isinstance(shape, dict) and "$ref" in shape:
                shape, path = _resolve(shape["$ref"], here)
                shape = rebase(shape, path)
            shape = flatten(shape, path)
            for k, pv in (shape.get("properties") or {}).items():
                if k in props:
                    if isinstance(pv, dict) and pv.get("required") and not pv.get("type") and not pv.get("$ref"):
                        base = props[k]
                        if "$ref" in base:
                            base, _ = _resolve(base["$ref"], here) if not base["$ref"].startswith("#") else _resolve(base["$ref"], here)
                            base = flatten(base, here)
                        merged = dict(base); merged["required"] = list(dict.fromkeys((base.get("required") or []) + pv["required"])); props[k] = merged
                    continue
                if isinstance(pv, dict) and not any(x in pv for x in ("type", "$ref", "oneOf", "enum", "properties", "const")):
                    continue
                props[k] = pv
            for r in shape.get("required") or []:
                if r not in required and r in props:
                    required.append(r)
        node["properties"] = props
        if required:
            node["required"] = required
    elif shapes:
        node["allOf"] = shapes
    return node


read = functools.cache(lambda uri: Resource.from_contents(flatten(_load(urllib.request.url2pathname(uri.removeprefix("file:"))), urllib.request.url2pathname(uri.removeprefix("file:")))))
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
