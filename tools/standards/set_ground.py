"""Point a body's `ground` at a supported snapshot in the worlds store, after checking it.

    python3 tools/standards/set_ground.py body.treistun.treistun-e worlds/TRE1/ground/earth_s3-20261009b [note]

Checks the snapshot's manifest: supported format, its world_id and body the record's, every listed file
present at its size; then writes {format, path, manifest_sha256, radius, note} into the record's `ground` block (replacing
one that is there). Run from the repository root.
"""
import glob, hashlib, json, math, os, re, sys
from pathlib import Path, PurePosixPath
import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
FORMATS = ("planet-unfold-tiles/1", "planet-land-pyramid/1")


def file_sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def validate_manifest(folder, key, world_id, verify_hashes=False):
    """Normal opens check sizes; installation may additionally verify all hashes."""
    folder = Path(folder).resolve()
    raw = (folder / "manifest.json").read_bytes()
    manifest = json.loads(raw)
    if manifest.get("format") not in FORMATS:
        raise ValueError(f"unsupported ground format {manifest.get('format')}")
    if manifest.get("body") not in (None, key) or manifest.get("world_id") not in (None, world_id):
        raise ValueError("ground manifest names a different body or survey world")
    land = manifest["format"] == "planet-land-pyramid/1"
    if land and (manifest.get("body") != key or manifest.get("world_id") != world_id):
        raise ValueError("land pyramid manifest must name its body and survey world")
    radius = manifest.get("radius_m")
    if isinstance(radius, bool) or not isinstance(radius, (int, float)) or not math.isfinite(radius) or radius <= 0:
        raise ValueError("ground radius_m must be finite and positive")
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        raise ValueError("ground manifest needs files")
    total = 0
    for name, entry in files.items():
        rel = PurePosixPath(name)
        if rel.is_absolute() or ".." in rel.parts or "\\" in name or not rel.parts or name == "manifest.json":
            raise ValueError(f"invalid ground file path {name!r}")
        path = folder / name
        if not path.resolve().is_relative_to(folder) or not path.is_file():
            raise ValueError(f"ground file missing or outside snapshot: {name}")
        if not isinstance(entry, dict):
            raise ValueError(f"invalid file entry {name}")
        size = entry.get("size")
        if land and (isinstance(size, bool) or not isinstance(size, int) or size < 0):
            raise ValueError(f"ground file needs its size: {name}")
        if size is not None and path.stat().st_size != size:
            raise ValueError(f"ground file has wrong size: {name}")
        digest = entry.get("sha256")
        if land and (not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest)):
            raise ValueError(f"ground file needs sha256: {name}")
        if verify_hashes and file_sha(path) != digest:
            raise ValueError(f"ground file has wrong sha256: {name}")
        total += path.stat().st_size
    return manifest, hashlib.sha256(raw).hexdigest(), total


def store():
    p = os.environ.get("UNIVERSE_WORLDS") or os.path.expanduser("~/git/planet-sim/out")
    return p if os.path.isdir(p) else sys.exit("no worlds store (UNIVERSE_WORLDS)")


def main(key, path, note=None):
    rec = next((f for f in glob.glob(os.path.join(ROOT, "standards/Celestial/metadata/systems/*/bodies/*.yaml")) if (yaml.safe_load(Path(f).read_text()) or {}).get("identity", {}).get("key") == key), None) or sys.exit(f"no body {key}")
    body = yaml.safe_load(Path(rec).read_text()); base = Path(store()).resolve(); folder = base / path
    if Path(path).is_absolute() or not folder.resolve().is_relative_to(base):
        sys.exit("ground snapshot must be relative to the worlds store")
    wid = (body.get("survey") or {}).get("world_id")
    m, sha, total = validate_manifest(folder, key, wid)
    note = note or f"{m.get('name', '')}: {len(m['files'])} files, {total / 1e6:.0f} MB in the worlds store" + (f"; terraform {m['source']['terraform']}" if (m.get("source") or {}).get("terraform") else "") + "."
    block = (f'ground:\n  format: {json.dumps(m["format"])}\n  path: {json.dumps(path)}\n  manifest_sha256: "{sha}"\n  radius: {m.get("radius_m")}\n  note: {json.dumps(note)}\n')
    s = Path(rec).read_text()
    s2 = re.sub(r"^ground:\n(?:  .*\n)+", block, s, count=1, flags=re.M) if re.search(r"^ground:\n", s, flags=re.M) else s.rstrip("\n") + "\n" + block
    yaml.safe_load(s2); Path(rec).write_text(s2)
    print(f"{key}: ground -> {path} ({len(m['files'])} files, {total / 1e6:.0f} MB, sha {sha[:12]})")


if __name__ == "__main__":
    if len(sys.argv) < 3: sys.exit(__doc__)
    main(*sys.argv[1:4])
