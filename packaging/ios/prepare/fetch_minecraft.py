#!/usr/bin/env python3
"""Downloads the pinned Minecraft: Java Edition files that horizon-prepare reads, from Mojang's
own download servers, and checks each against research-lock.json and Mojang's checksums.

For a player who owns Java Edition and has no computer to prepare on: the prepare-game-files
workflow runs this in the player's own PRIVATE repository (packaging/ios/README.md). Nothing it
downloads may be shared: Horizon ships none of Mojang's files.

    fetch_minecraft.py ROOT OUT

ROOT is a Horizon checkout (its research-lock.json pins the version and the jars). OUT gets
version.json, client.jar, server.jar, libraries/ (Maven layout) and assets/ (indexes/, objects/).
"""
import concurrent.futures
import hashlib
import json
import pathlib
import sys
import urllib.request

MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "horizon-prepare-ci"})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                return response.read()
        except OSError:
            if attempt == 3:
                raise
    raise AssertionError


def sha1(data: bytes) -> str:
    return hashlib.sha1(data).hexdigest()


def save(url: str, path: pathlib.Path, expected: str | None) -> None:
    if path.is_file() and (expected is None or sha1(path.read_bytes()) == expected):
        return
    data = fetch(url)
    if expected is not None and sha1(data) != expected:
        raise SystemExit(f"{url}: sha1 {sha1(data)}, expected {expected}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def main() -> None:
    root, out = map(pathlib.Path, sys.argv[1:3])
    lock = json.loads((root / "research-lock.json").read_text())["minecraft"]
    accepted = {lock["manifest_sha1"], *lock.get("accepted_manifest_sha1", [])}
    manifest = json.loads(fetch(MANIFEST))
    entry = next(v for v in manifest["versions"] if v["id"] == lock["version"])
    if entry["sha1"] not in accepted:
        print(f"note: Mojang's {lock['version']} JSON is {entry['sha1']}, not one research-lock.json "
              "lists; the jars are still checked against it")
    version_bytes = fetch(entry["url"])
    if sha1(version_bytes) != entry["sha1"]:
        raise SystemExit("the version JSON does not match Mojang's manifest")
    out.mkdir(parents=True, exist_ok=True)
    (out / "version.json").write_bytes(version_bytes)
    version = json.loads(version_bytes)

    downloads = version["downloads"]
    if downloads["client"]["sha1"] != lock["client_sha1"] or downloads["server"]["sha1"] != lock["server_sha1"]:
        raise SystemExit("Mojang's jars are not the ones research-lock.json pins")
    jobs = [
        (downloads["client"]["url"], out / "client.jar", lock["client_sha1"]),
        (downloads["server"]["url"], out / "server.jar", lock["server_sha1"]),
    ]
    for library in version["libraries"]:
        artifact = library.get("downloads", {}).get("artifact")
        # Every platform's: a few megabytes more, and nothing the checks look for is missing.
        if artifact:
            jobs.append((artifact["url"], out / "libraries" / artifact["path"], artifact["sha1"]))

    index = version["assetIndex"]
    index_bytes = fetch(index["url"])
    if sha1(index_bytes) != index["sha1"]:
        raise SystemExit("the asset index does not match the version JSON")
    index_path = out / "assets" / "indexes" / f"{index['id']}.json"
    index_path.parent.mkdir(parents=True, exist_ok=True)
    index_path.write_bytes(index_bytes)
    for obj in json.loads(index_bytes)["objects"].values():
        h = obj["hash"]
        jobs.append((f"https://resources.download.minecraft.net/{h[:2]}/{h}",
                     out / "assets" / "objects" / h[:2] / h, h))

    print(f"downloading {len(jobs)} files")
    with concurrent.futures.ThreadPoolExecutor(16) as pool:
        for done, _ in enumerate(pool.map(lambda job: save(*job), jobs), 1):
            if done % 500 == 0:
                print(f"  {done}/{len(jobs)}")
    print("done")


if __name__ == "__main__":
    main()
