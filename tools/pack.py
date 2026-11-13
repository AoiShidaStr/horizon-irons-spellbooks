#!/usr/bin/env python3
"""Packs content/ into target/irons_spellbooks.hmod.

A .hmod is a zip of `horizon-mod.toml`, `assets/` and `data/` at its root; this
is what `cargo horizon-mod build` writes, so the file can be checked on the
build VPS with `cargo-horizon-mod horizon-mod check`.

Run from the mod folder: python tools/pack.py
"""

import os
import subprocess
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONTENT = os.path.join(ROOT, "content")
OUT = os.path.join(ROOT, "target", "irons_spellbooks.hmod")

REQUIRED = ["horizon-mod.toml", "assets", "data"]
MAX_FILE = 16 * 1024 * 1024


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
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
