#!/usr/bin/env python3
"""Read the monorepo release version and verify its derived packaging metadata."""

import argparse
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib


ROOT = Path(__file__).resolve().parents[2]
ARCH = ROOT / "packaging/arch"
RECIPES = (ARCH / "memedock-git/PKGBUILD", ARCH / "memedock-bin/PKGBUILD")


def metadata():
    with (ROOT / "Cargo.toml").open("rb") as source:
        workspace = tomllib.load(source)["workspace"]
    version = workspace["package"]["version"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Release version must be stable major.minor.patch")
    major, minor, patch = map(int, version.split("."))
    if max(major, minor, patch) > 999:
        raise ValueError("Release version components must be <= 999")
    code = major * 1_000_000 + minor * 1_000 + patch
    if code == 0:
        raise ValueError("Android versionCode must be positive")
    with (ROOT / "rust-toolchain.toml").open("rb") as source:
        toolchain = tomllib.load(source)["toolchain"]["channel"]
    if toolchain != workspace["package"]["rust-version"]:
        raise ValueError("Rust toolchain and workspace rust-version must agree")
    return {"version": version, "version_code": code, "tag": f"v{version}", "rust": toolchain}


def archive(destination, version):
    # Include source edits and new, non-ignored files so local packaging can be
    # verified before committing. Git ignores keep credentials/build data out.
    names = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
    ).decode().split("\0")
    destination = destination.resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(destination, "w:gz") as output:
        for name in sorted(set(filter(None, names))):
            path = ROOT / name
            if path == destination or not (path.is_file() or path.is_symlink()):
                continue
            output.add(path, arcname=f"MemeDock-{version}/{name}", recursive=False)
        count = subprocess.check_output(["git", "rev-list", "--count", "HEAD"], cwd=ROOT).decode().strip()
        revision = subprocess.check_output(["git", "rev-parse", "--short=7", "HEAD"], cwd=ROOT).decode().strip()
        snapshot = f"{version}.r{count}.g{revision}\n".encode()
        info = tarfile.TarInfo(f"MemeDock-{version}/.memedock-git-version")
        info.size = len(snapshot)
        info.mode = 0o644
        output.addfile(info, io.BytesIO(snapshot))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=("json", "version", "code", "rust", "github"), default="version")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--sync-arch", action="store_true")
    parser.add_argument("--tag", help="Require this tag to match the root version")
    parser.add_argument("--archive", type=Path, help="Create a source archive for local/CI makepkg")
    args = parser.parse_args()
    value = metadata()
    if args.tag is not None and args.tag != value["tag"]:
        parser.error(f"Tag {args.tag!r} does not match {value['tag']!r}")
    expected = f"pkgver={value['version']}"
    for recipe in RECIPES:
        text = recipe.read_text()
        if args.sync_arch:
            text, count = re.subn(r"^pkgver=.*$", expected, text, flags=re.MULTILINE)
            if count != 1:
                parser.error(f"{recipe} must contain exactly one pkgver field")
            recipe.write_text(text)
        if args.check:
            versions = re.findall(r"^pkgver=(.*)$", text, re.MULTILINE)
            pattern = re.escape(value["version"])
            if recipe.parent.name == "memedock-git":
                pattern += r"(?:\.r[0-9]+\.g[0-9a-f]+)?"
            if len(versions) != 1 or not re.fullmatch(pattern, versions[0]):
                parser.error(f"{recipe} version differs; run tools/release/version.py --sync-arch")
    if args.check:
        with (ROOT / "Cargo.toml").open("rb") as source:
            members = tomllib.load(source)["workspace"]["members"]
        for member in members:
            with (ROOT / member / "Cargo.toml").open("rb") as source:
                if tomllib.load(source)["package"]["version"] != {"workspace": True}:
                    parser.error(f"{member} must inherit the workspace version")
    if args.archive:
        archive(args.archive, value["version"])
    if args.format == "github":
        output = os.environ.get("GITHUB_OUTPUT")
        if not output:
            parser.error("GITHUB_OUTPUT is required for --format github")
        with open(output, "a") as stream:
            for key, item in value.items():
                stream.write(f"{key}={item}\n")
    elif args.format == "json":
        print(json.dumps(value))
    else:
        key = {"code": "version_code", "rust": "rust"}.get(args.format, "version")
        print(value[key])


if __name__ == "__main__":
    main()
