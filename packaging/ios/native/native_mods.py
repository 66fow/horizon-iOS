#!/usr/bin/env python3
"""Native mods for the iOS build (packaging/ios/native-mods.md): the checklist issue and the downloads.

    native_mods.py checklist INDEX BODY_IN BODY_OUT   # the issue body: every native mod, ticks kept
    native_mods.py picked BODY                         # the ticked mod ids, one per line
    native_mods.py fetch INDEX OUT ID... [--user RELEASES]  # download, check and unpack each into OUT/<id>/;
                                                       # ids not in the registry come from the fork's own
                                                       # mod-<id>-v<version> releases (the mod maker's)

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


USER_MARK = re.compile(r"<!-- horizon-user-mod (\{.*?\}) -->")


def user_mods(releases: list[dict]) -> dict[str, dict]:
    """The fork's own native mods (ios-user-mod.yml's releases), newest release of each id:
    {id: {"version", "url", "size", "sha256"}}."""
    out: dict[str, dict] = {}
    for r in sorted(releases, key=lambda r: r.get("created_at", "")):
        m = USER_MARK.search(r.get("body") or "")
        if not r.get("tag_name", "").startswith("mod-") or not m:
            continue
        meta = json.loads(m.group(1))
        if meta.get("kind") != "native":
            continue
        asset = next((a for a in r.get("assets", []) if a["name"] == f"{meta['id']}.hnmod"), None)
        if asset:
            out[meta["id"]] = {"version": meta["version"], "url": asset["browser_download_url"],
                               "size": meta["size"], "sha256": meta["sha256"], "name": meta["id"]}
    return out


def fetch(index: dict, out: pathlib.Path, ids: list[str], user: dict[str, dict] | None = None) -> dict:
    by_id = {p["id"]: p for p in native(index)}
    user = user or {}
    report = {"fetched": [], "missing": []}
    for mod in ids:
        p = by_id.get(mod)
        if p is not None:
            v = p["versions"][0]
            f = v["file"]
            url, size, sha = f["url"], int(f["size"]), f["sha256"]
            name, version = p["name"], v["version"]
        elif mod in user:
            u = user[mod]
            url, size, sha, name, version = u["url"], int(u["size"]), u["sha256"], u["name"], u["version"]
        else:
            report["missing"].append({"id": mod, "why": "not a native mod for 26.3 in the registry or this fork"})
            continue
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
        report["fetched"].append({"id": mod, "version": version, "name": name})
    return report


# Mods made for an older Horizon whose hooks have since moved, as (id, version) -> [(file, old, new)]:
# each points them at where those functions are now, until their authors update them. Only the exact
# version listed is touched, and a fix whose old text isn't there fails the build instead of guessing.
FIXES: dict[tuple[str, str], list[tuple[str, str, str]]] = {
    # Streaming::forget became forget_columns (a batch of columns), gizmos moved to the client lib,
    # and the chat's show_suggestions takes whether Tab asked.
    ("baritone", "1.1.38"): [
        ("horizon-mixin.toml",
         'target = "horizon_client_render::streaming::Streaming::forget"\n'
         'helper = "hooks/unload.rs"\n'
         'bindings = [\n'
         '  { name = "world", ty = "&World", value = "&*world" },\n'
         '  { name = "x", ty = "i32", value = "x" },\n'
         '  { name = "z", ty = "i32", value = "z" },\n'
         ']',
         'target = "horizon_client_render::streaming::Streaming::forget_columns"\n'
         'helper = "hooks/unload.rs"\n'
         'bindings = [\n'
         '  { name = "world", ty = "&World", value = "&*world" },\n'
         '  { name = "gone", ty = "&FxHashSet<(i32, i32)>", value = "gone" },\n'
         ']'),
        ("horizon-mixin.toml",
         'target = "horizon_client::gizmos::camera_relative"',
         'target = "horizon_client_lib::gizmos::camera_relative"'),
        # show_suggestions took `immediate` (Tab asked for them): false as Horizon passes while typing.
        ("hooks/suggest_hook.rs",
         "        screen.show_suggestions(m);",
         "        screen.show_suggestions(m, false);"),
        ("hooks/unload.rs",
         'fn baritone_unload(world: &World, x: i32, z: i32) {\n'
         '    if let Some(chunk) = world.chunks.get(&(x, z)) {\n'
         '        baritone_unload::push(x, z, chunk.clone());\n'
         '    }\n'
         '}',
         'fn baritone_unload(world: &World, gone: &FxHashSet<(i32, i32)>) {\n'
         '    for &(x, z) in gone {\n'
         '        if let Some(chunk) = world.chunks.get(&(x, z)) {\n'
         '            baritone_unload::push(x, z, chunk.clone());\n'
         '        }\n'
         '    }\n'
         '}'),
    ],
    # The server's particles have a speed per axis, block loot takes the block entity (none for logs
    # and leaves), and a hit says which species struck (none for a falling tree).
    ("chop", "1.0.1"): [
        ("server.rs",
         "                spread,\n                speed,\n                count: ((base * k).round() as i32).max(1),",
         "                spread,\n                speed: [speed; 3],\n                count: ((base * k).round() as i32).max(1),"),
        ("server.rs",
         "game.block_loot_at(s.cut, s.cut_state, s.tool.as_ref())",
         "game.block_loot_at(s.cut, s.cut_state, s.tool.as_ref(), None)"),
        ("server.rs",
         "game.block_loot_at(part.cell, id, s.tool.as_ref())",
         "game.block_loot_at(part.cell, id, s.tool.as_ref(), None)"),
        ("server.rs",
         "game.block_loot_at(*h, id, s.tool.as_ref())",
         "game.block_loot_at(*h, id, s.tool.as_ref(), None)"),
        ("server.rs",
         "game.block_loot_at(leaf, id, None)",
         "game.block_loot_at(leaf, id, None, None)"),
        ("server.rs",
         "                    from: Some([pos[0] - dir[0], pos[1], pos[2] - dir[1]]),\n                },",
         "                    from: Some([pos[0] - dir[0], pos[1], pos[2] - dir[1]]),\n"
         "                    attacker_species: None,\n                },"),
    ],
    # The title screen's buttons have a controller `action` now: none for the Mods button.
    ("mod_menu", "0.3.3"): [
        ("title_buttons.rs",
         "                icon: None,\n                trailing: None,\n            });",
         "                icon: None,\n                trailing: None,\n                action: None,\n            });"),
    ],
    # The server's lan module moved from horizon-client to horizon-client-lib.
    ("shulker_peek", "0.1.0"): [
        ("horizon-mixin.toml",
         'target = "horizon_client::lan::hub::Hub::stack"',
         'target = "horizon_client_lib::lan::hub::Hub::stack"'),
        # The tooltip's box is a method now, and takes the stack (its rarity's border colours).
        ("tooltip.rs",
         "    crate::game::Game::tooltip_box(batch, atlas, at.x, at.y, at.w, at.h);",
         "    game.tooltip_box(batch, Some(stack), at.x, at.y, at.w, at.h);"),
    ],
}


def fix(out: pathlib.Path, fetched: list[dict]) -> list[str]:
    """Applies FIXES to the mods fetch unpacked into OUT/<id>/, keeping each file's line endings."""
    done = []
    for mod in fetched:
        edits = FIXES.get((mod["id"], mod["version"]))
        if not edits:
            continue
        root = out / mod["id"] / "mixins" / mod["id"]
        for name, old, new in edits:
            path = root / name
            raw = path.read_bytes().decode("utf-8")
            crlf = "\r\n" in raw
            text = raw.replace("\r\n", "\n")
            if text.count(old) != 1:
                raise SystemExit(f"{mod['id']} {mod['version']}: the fix for {name} no longer fits")
            text = text.replace(old, new)
            path.write_bytes((text.replace("\n", "\r\n") if crlf else text).encode("utf-8"))
        done.append(f"{mod['id']} {mod['version']}")
    return done


def main() -> None:
    cmd, *args = sys.argv[1:]
    if cmd == "checklist":
        index = json.loads(pathlib.Path(args[0]).read_text())
        old = pathlib.Path(args[1]).read_text() if pathlib.Path(args[1]).exists() else ""
        pathlib.Path(args[2]).write_text(checklist(index, old))
    elif cmd == "picked":
        print("\n".join(picked(pathlib.Path(args[0]).read_text())))
    elif cmd == "fetch":
        user = None
        if "--user" in args:
            at = args.index("--user")
            user = user_mods(json.loads(pathlib.Path(args[at + 1]).read_text()))
            args = args[:at] + args[at + 2:]
        index = json.loads(pathlib.Path(args[0]).read_text())
        out = pathlib.Path(args[1])
        out.mkdir(parents=True, exist_ok=True)
        report = fetch(index, out, args[2:], user)
        report["fixed"] = fix(out, report["fetched"])
        print(json.dumps(report, indent=1))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
