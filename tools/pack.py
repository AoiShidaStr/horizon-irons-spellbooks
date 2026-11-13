#!/usr/bin/env python3
"""Packs the mod's two files, as docs/api/mod-files.md describes them.

  target/irons_spellbooks.hmod   horizon-mod.toml + assets/ + data/
  target/irons_spellbooks.hnmod  horizon-native.json + mixins/irons_spellbooks/...

Run from the mod folder: python tools/pack.py
"""

import hashlib
import json
import os
import subprocess
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONTENT = os.path.join(ROOT, "content")
OUT = os.path.join(ROOT, "target", "irons_spellbooks.hmod")
NATIVE_OUT = os.path.join(ROOT, "target", "irons_spellbooks.hnmod")
BUNDLE = os.path.join(ROOT, "mixins", "irons_spellbooks")
ID = "irons_spellbooks"

# The revision the bundle pins and was checked against, the horizon-mixins
# commit it builds with (as the sibling ports pin it), and the game version.
HORIZON = "a85af177a82504f357ff10d5d40d0b7230e2ac32"
MIXINS = "5a64af938ec86b00d53d16a408aabc0038fb3e57"
GAME = "26.3"
# NATIVE_HOOKS_API of the pinned commit (crates/horizon-client/src/game/native_hooks.rs).
HOOKS = 415

REQUIRED = ["horizon-mod.toml", "assets", "data"]
MAX_FILE = 16 * 1024 * 1024


def version():
    with open(os.path.join(CONTENT, "horizon-mod.toml"), encoding="utf-8") as handle:
        for line in handle:
            if line.strip().startswith("version"):
                return line.split("=", 1)[1].strip().strip('"')
    raise SystemExit("no version in content/horizon-mod.toml")


def pack_native():
    """The bundle, with its manifest: horizon-mixin.toml and api/ at its root."""
    native = {
        "schema": 1,
        "id": ID,
        "version": version(),
        "game": GAME,
        "horizon": HORIZON,
        "mixins": MIXINS,
        "hooks": HOOKS,
    }
    with zipfile.ZipFile(NATIVE_OUT, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        info = zipfile.ZipInfo("horizon-native.json", date_time=(2026, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        archive.writestr(info, json.dumps(native, indent=2) + "\n")
        for base, dirs, files in os.walk(BUNDLE):
            dirs[:] = sorted(d for d in dirs if d not in ("target", ".build"))
            for file in sorted(files):
                full = os.path.join(base, file)
                if os.path.getsize(full) > MAX_FILE:
                    print(f"too big: {full}", file=sys.stderr)
                    return 1
                rel = os.path.relpath(full, BUNDLE).replace(os.sep, "/")
                archive.write(full, f"mixins/{ID}/{rel}")
    with open(NATIVE_OUT, "rb") as handle:
        digest = hashlib.sha256(handle.read()).hexdigest()
    print(f"{NATIVE_OUT} ({os.path.getsize(NATIVE_OUT)} bytes, sha256 {digest})")
    return 0


def main():
    if subprocess.call([sys.executable, os.path.join(ROOT, "tools", "gen_content.py")]) != 0:
        return 1
    for name in REQUIRED:
        if not os.path.exists(os.path.join(CONTENT, name)):
            print(f"missing {name} in {CONTENT}", file=sys.stderr)
            return 1

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    count = 0
    with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        archive.write(os.path.join(CONTENT, "horizon-mod.toml"), "horizon-mod.toml")
        for folder in ("assets", "data"):
            for base, _, files in os.walk(os.path.join(CONTENT, folder)):
                for file in sorted(files):
                    full = os.path.join(base, file)
                    if os.path.getsize(full) > MAX_FILE:
                        print(f"too big: {full}", file=sys.stderr)
                        return 1
                    name = os.path.relpath(full, CONTENT).replace(os.sep, "/")
                    archive.write(full, name)
                    count += 1
    print(f"{OUT} ({os.path.getsize(OUT)} bytes, {count + 1} files)")
    return pack_native()


if __name__ == "__main__":
    raise SystemExit(main())
