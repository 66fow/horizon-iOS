"""The iOS native builds take the mod maker's own native mods from the fork's releases."""
import importlib.util
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("native_mods", ROOT / "packaging/ios/native/native_mods.py")
native_mods = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native_mods)


def release(tag, meta, created, asset=None):
    body = "notes\n\n<!-- horizon-user-mod " + json.dumps(meta) + " -->"
    name = asset or f"{meta['id']}.hnmod"
    return {"tag_name": tag, "body": body, "created_at": created,
            "assets": [{"name": name, "browser_download_url": f"https://github.com/o/r/releases/download/{tag}/{name}"}]}


class UserMods(unittest.TestCase):
    def test_the_newest_native_release_of_each_mod_is_used(self):
        old = {"id": "zoom", "kind": "native", "version": "0.1.0", "sha256": "a" * 64, "size": 10}
        new = dict(old, version="0.2.0", sha256="b" * 64)
        mods = native_mods.user_mods([
            release("mod-zoom-v0.2.0", new, "2026-11-07T12:00:00Z"),
            release("mod-zoom-v0.1.0", old, "2026-11-06T12:00:00Z"),
        ])
        self.assertEqual(mods["zoom"]["version"], "0.2.0")
        self.assertEqual(mods["zoom"]["sha256"], "b" * 64)
        self.assertTrue(mods["zoom"]["url"].endswith("/mod-zoom-v0.2.0/zoom.hnmod"))

    def test_content_mods_and_other_releases_are_left_out(self):
        content = {"id": "gems", "kind": "content", "version": "1", "sha256": "c" * 64, "size": 1}
        mods = native_mods.user_mods([
            release("mod-gems-v1", content, "2026-11-07T12:00:00Z", asset="gems.hmod"),
            {"tag_name": "ios-r22", "body": "Horizon for iOS", "created_at": "2026-11-07T00:00:00Z", "assets": []},
        ])
        self.assertEqual(mods, {})


if __name__ == "__main__":
    unittest.main()
