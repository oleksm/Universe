"""Snapshot a planet LOD anchor/land-pyramid package and set a body's ground.

Dry run by default; --apply copies into the worlds store and changes only ground.
Comparison pyramids are excluded. Source bytes and package-relative paths are
preserved, and a content-named snapshot is published only after hash verification.
"""
import argparse
import contextlib
import hashlib
import io
import json
import math
import os
from pathlib import Path
import re
import shutil
import tempfile
import yaml
import set_ground

FORMAT = "planet-land-pyramid/1"
EXCLUDED = {"pyramid2", "pyramid_600belt", "pyramid_nobelt"}
REQUIRED = {"info.json", "lod.json", "dir.f32", "z.f32", "kind.u8", "tri.u32", "pyramid/pyramid.json"}


def inventory(source):
    """No symlinks, no comparison copies, and no reserved snapshot manifest."""
    result = {}
    for folder, dirs, files in os.walk(source):
        if Path(folder) == source:
            dirs[:] = [name for name in dirs if name not in EXCLUDED]
        for name in dirs + files:
            path = Path(folder) / name
            if path.is_symlink():
                raise ValueError(f"source symlink is not a snapshot input: {path}")
        for name in files:
            path = Path(folder) / name
            relative = path.relative_to(source).as_posix()
            if relative == "manifest.json":
                raise ValueError("source already contains a reserved snapshot manifest")
            stat = path.stat()
            result[relative] = (stat.st_size, stat.st_mtime_ns, stat.st_ino)
    return dict(sorted(result.items()))


def check_source(source, files):
    if REQUIRED - files.keys():
        raise ValueError(f"missing source package files: {sorted(REQUIRED - files.keys())}")
    info = json.loads((source / "info.json").read_text())
    radius = info.get("radius_m")
    if isinstance(radius, bool) or not isinstance(radius, (int, float)) or not math.isfinite(radius) or radius <= 0:
        raise ValueError("info.json needs a finite positive radius_m")
    pyramid = json.loads((source / "pyramid/pyramid.json").read_text())
    if pyramid.get("format") != "planet-forge land pyramid 1" or pyramid.get("projection", {}).get("radius_m") != radius:
        raise ValueError("pyramid format/radius disagrees with the anchor package")
    levels = pyramid.get("levels", [])
    if not levels or [item.get("k") for item in levels] != list(range(pyramid.get("top_level", -1) + 1)):
        raise ValueError("pyramid levels must cover 0 through top_level in order")
    for level in levels:
        window = level.get("window")
        rows, cols = (window["rows"], window["cols"]) if window else (level["n"], level["n"])
        if not all(isinstance(n, int) and not isinstance(n, bool) and n > 0 for n in (rows, cols)):
            raise ValueError("pyramid raster/window must have positive integer dimensions")
        for field, width in (("z", 4), ("rgb", 3), ("mask", 1)):
            name = "pyramid/" + level[field]
            if name not in files or files[name][0] != rows * cols * width:
                raise ValueError(f"pyramid array has wrong size or is missing: {name}")
    return info


def install(source, key, world=None, apply=False, snapshot_name=None, source_commit=None):
    source = Path(source).expanduser().resolve()
    if not source.is_dir():
        raise ValueError(f"no package directory: {source}")
    root = Path(set_ground.ROOT)
    matches = [path for path in (root / "standards/Celestial/metadata/systems").glob("*/bodies/*.yaml")
               if yaml.safe_load(path.read_text())["identity"]["key"] == key]
    if len(matches) != 1:
        raise ValueError(f"expected one body record for {key}")
    record = matches[0]
    before = record.read_text()
    body = yaml.safe_load(before)
    survey_world = (body.get("survey") or {}).get("world_id")
    world = world or survey_world
    if not world or world != survey_world or not re.fullmatch(r"[A-Za-z0-9_-]+", world):
        raise ValueError("ground install must keep the body's existing survey world")
    if body.get("bake"):
        raise ValueError("body has a surface bake; resolve it explicitly before installing ground")
    if snapshot_name and not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", snapshot_name):
        raise ValueError("snapshot name must be a single ordinary folder name")
    files = inventory(source)
    info = check_source(source, files)
    summary = {"format": FORMAT, "body": key, "world_id": world, "files": len(files),
               "bytes": sum(value[0] for value in files.values()), "radius_m": info["radius_m"]}
    if not apply:
        return dict(summary, dry_run=True, excluded=sorted(EXCLUDED))

    store = Path(set_ground.store()).resolve()
    parent = store / "worlds" / world / "ground"
    parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix=".install-ground-", dir=parent))
    try:
        entries = {}
        for name in files:
            src, target = source / name, stage / name
            target.parent.mkdir(parents=True, exist_ok=True)
            digest = hashlib.sha256()
            with src.open("rb") as incoming, target.open("wb") as outgoing:
                for block in iter(lambda: incoming.read(1 << 20), b""):
                    outgoing.write(block)
                    digest.update(block)
            entries[name] = {"sha256": digest.hexdigest(), "size": target.stat().st_size}
        if inventory(source) != files:
            raise ValueError("source package changed during snapshot; no record updated")
        tree_hash = hashlib.sha256(json.dumps(entries, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        name = snapshot_name or f"{source.name}-{tree_hash[:12]}"
        relative = f"worlds/{world}/ground/{name}"
        destination = store / relative
        manifest = {"format": FORMAT, "world_id": world, "body": key, "name": source.name,
                    "radius_m": info["radius_m"], "source": {"package": source.name,
                    "info": info.get("source"), "fileset_sha256": tree_hash,
                    **({"commit": source_commit} if source_commit else {})}, "files": entries}
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=1) + "\n")
        set_ground.validate_manifest(stage, key, world, verify_hashes=True)
        reused = destination.exists()
        if reused:
            old, _, _ = set_ground.validate_manifest(destination, key, world, verify_hashes=True)
            actual = {p.relative_to(destination).as_posix() for p in destination.rglob("*") if p.is_file()}
            if (old["format"] != FORMAT or old["radius_m"] != manifest["radius_m"] or old["files"] != entries
                    or actual != set(entries) | {"manifest.json"} or any((destination / n).exists() for n in EXCLUDED)):
                raise ValueError("immutable snapshot name already contains different data")
        else:
            stage.rename(destination)
        note = (f"{source.name}: planet LOD anchor and live land pyramid: {len(entries)} files, "
                f"{summary['bytes']} bytes; comparison pyramids excluded; survey {world} retained.")
        if record.read_text() != before:
            raise ValueError("body record changed during snapshot; ground not updated")
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                set_ground.main(key, relative, note)
            after = yaml.safe_load(record.read_text())
            if {k: v for k, v in after.items() if k != "ground"} != {k: v for k, v in body.items() if k != "ground"}:
                raise ValueError("ground install unexpectedly changed other body fields")
        except BaseException:
            record.write_text(before)
            raise
        digest = set_ground.file_sha(destination / "manifest.json")
        return dict(summary, path=relative, manifest_sha256=digest, reused=reused)
    finally:
        if stage.exists():
            shutil.rmtree(stage)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source")
    parser.add_argument("body")
    parser.add_argument("--world")
    parser.add_argument("--snapshot-name")
    parser.add_argument("--source-commit")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    result = install(args.source, args.body, args.world, args.apply, args.snapshot_name, args.source_commit)
    print(json.dumps(result, indent=2) if args.json else f"{result}")


if __name__ == "__main__":
    main()
