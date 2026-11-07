#!/usr/bin/env python3
"""Native mods for the iOS build (packaging/ios/native-mods.md): the checklist issue and the downloads.

    native_mods.py checklist INDEX BODY_IN BODY_OUT   # the issue body: every native mod, ticks kept
    native_mods.py picked BODY                         # the ticked mod ids, one per line
    native_mods.py fetch INDEX OUT ID...               # download, check and unpack each into OUT/<id>/

INDEX is the registry's index.json (https://horizon-rust.github.io/horizon-mods/index.json). Only
the registry's own files are downloaded (GitHub release assets over https), each checked against its
SHA-256 and size before it is unpacked; nothing in them is run here.
"""
import hashlib
import json
import pathlib
import re
import sys
import urllib.request
import zipfile

GAME = "26.3"
BUILD = "Build now"
TITLE = "Native mods for iOS"
MAX_FILE = 64 * 1024 * 1024
TICK = re.compile(r"^- \[([ xX])\] `([a-z0-9_.-]+)`")


def native(index: dict) -> list[dict]:
    """The native mods with a version for this game, by name."""
    out = []
    for p in index["projects"]:
        if p.get("kind") != "native-mod":
            continue
        versions = [v for v in p.get("versions", []) if v.get("game") == GAME and not v.get("revoked")]
        if versions:
            out.append({**p, "versions": versions})
    return sorted(out, key=lambda p: p["name"].lower())


def picked(body: str) -> list[str]:
    return [m.group(2) for line in body.splitlines() if (m := TICK.match(line.strip())) and m.group(1) in "xX"]


def checklist(index: dict, old: str) -> str:
    keep = set(picked(old))
    lines = [
        f"Tick the native mods to build into Horizon for iOS, then tick **{BUILD}**. GitHub builds an",
        "`.ipa` with them (about 30-45 minutes) and links it here. The Horizon app's Get mods > Native",
        "tab ticks the same list.",
        "",
        "> Native mods change the game's own code and run with full access to the app (not sandboxed).",
        "> The registry reviews each one before listing it. Ticking one means you trust its code.",
        "",
        f"- [ ] **{BUILD}**",
        "",
    ]
    for p in native(index):
        mark = "x" if p["id"] in keep else " "
        summary = p.get("summary", "").replace("\n", " ").strip()
        lines.append(f"- [{mark}] `{p['id']}` {p['name']}: {summary}")
    return "\n".join(lines) + "\n"


def fetch(index: dict, out: pathlib.Path, ids: list[str]) -> dict:
    by_id = {p["id"]: p for p in native(index)}
    report = {"fetched": [], "missing": []}
    for mod in ids:
        p = by_id.get(mod)
        if p is None:
            report["missing"].append({"id": mod, "why": "not a native mod for 26.3 in the registry"})
            continue
        v = p["versions"][0]
        f = v["file"]
        url, size, sha = f["url"], int(f["size"]), f["sha256"]
        if not url.startswith("https://github.com/") or "/releases/download/" not in url or size > MAX_FILE:
            report["missing"].append({"id": mod, "why": f"unexpected file {url} ({size} bytes)"})
            continue
        req = urllib.request.Request(url, headers={"User-Agent": "horizon-ios-native-build"})
        with urllib.request.urlopen(req, timeout=120) as r:
            data = r.read(MAX_FILE + 1)
        if len(data) != size or hashlib.sha256(data).hexdigest() != sha:
            report["missing"].append({"id": mod, "why": "size or SHA-256 doesn't match the registry"})
            continue
        dest = out / mod
        dest.mkdir(parents=True, exist_ok=True)
        archive = out / f".{mod}.hnmod"
        archive.write_bytes(data)
        with zipfile.ZipFile(archive) as z:
            for name in z.namelist():
                target = (dest / name).resolve()
                if not str(target).startswith(str(dest.resolve()) + "/") and target != dest.resolve():
                    raise SystemExit(f"{mod}: {name} would land outside its folder")
            z.extractall(dest)
        archive.unlink()
        report["fetched"].append({"id": mod, "version": v["version"], "name": p["name"]})
    return report


def main() -> None:
    cmd, *args = sys.argv[1:]
    if cmd == "checklist":
        index = json.loads(pathlib.Path(args[0]).read_text())
        old = pathlib.Path(args[1]).read_text() if pathlib.Path(args[1]).exists() else ""
        pathlib.Path(args[2]).write_text(checklist(index, old))
    elif cmd == "picked":
        print("\n".join(picked(pathlib.Path(args[0]).read_text())))
    elif cmd == "fetch":
        index = json.loads(pathlib.Path(args[0]).read_text())
        out = pathlib.Path(args[1])
        out.mkdir(parents=True, exist_ok=True)
        print(json.dumps(fetch(index, out, args[2:]), indent=1))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
