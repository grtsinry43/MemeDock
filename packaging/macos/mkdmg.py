#!/usr/bin/env python3
"""Build the installer's disk image from a staging folder, without mounting one.

dmgbuild does the same job by attaching a scratch read-write image, copying the payload into
the mount point, writing the .DS_Store there and converting the result to UDZO. That is the
path make-dmg.sh normally takes, and it is kept as the default.

This exists because that path cannot run everywhere. It needs to write to /Volumes, which a
sandbox, a locked-down machine or some CI containers refuse, and it fails at the very last
step: hdiutil reports success, a .dmg appears, and it holds no application at all. The
alternative is to lay the volume's contents out in a folder and hand that to
`hdiutil create -srcfolder`, which mounts nothing.

The one thing that needs care is the background alias. Finder follows it to find the picture
inside the volume, so it has to name the volume the file will live in, which is a path that
does not exist while this runs. Building it against the staging folder instead would record
that folder's CNID and carbon path, neither of which resolves once mounted, and the window
comes up with no background and no obvious reason why.

Usage:
    mkdmg.py --app <MemeDock.app> --background <bg.png> --volume <Name> --out <out.dmg>
"""

import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile


def build_alias(volume, filename):
    """The alias Finder follows to the background picture inside the volume.

    Built for the volume rather than for the staging folder it is made in. An alias from
    mac_alias.Alias.for_file carries the CNID path and the carbon path of whatever filesystem
    it was made on; mounted as a volume image those resolve to nothing, so the fields are
    cleared and only the two Finder actually matches on are set, the volume name and the
    file's name in it.
    """
    from mac_alias import Alias

    alias = Alias.for_file("/")
    alias.target.filename = filename.encode()
    alias.target.folder_name = volume.encode()
    alias.target.creator_code = b"\x00\x00\x00\x00"
    alias.target.type_code = b"\x00\x00\x00\x00"
    alias.target.cnid_path = []
    alias.target.carbon_path = None
    alias.target.posix_path = None
    alias.volume.name = volume.encode()
    return alias.to_bytes()


def write_ds_store(path, volume, app_name, icon_size, label_size, app_at, applications_at):
    """The .DS_Store that carries the window: its size and position, the icon size, where each
    icon sits, and the background picture. dmgbuild writes this into a mounted volume; written
    beside the payload before the image is made, it lands in the same place."""
    from ds_store import DSStore

    store = DSStore.open(str(path), "w+")
    try:
        store["."]["bwsp"] = {
            "WindowBounds": "{{200, 200}, {640, 460}}",
            "ShowStatusBar": False,
            "ShowTabView": False,
            "ShowToolbar": False,
            "ShowPathbar": False,
            "ShowSidebar": False,
            "SidebarWidth": 0,
            "PreviewPaneVisibility": False,
            "ContainerShowSidebar": False,
        }
        store["."]["icvp"] = {
            "viewOptionsVersion": 1,
            "backgroundType": 2,
            "backgroundImageAlias": build_alias(volume, ".background.png"),
            "iconSize": icon_size,
            "textSize": label_size,
            "arrangeBy": "none",
            "showItemInfo": False,
            "showIconPreview": False,
            "labelOnBottom": True,
            "gridSpacing": 100.0,
            "gridOffsetX": 0.0,
            "gridOffsetY": 0.0,
            "scrollPositionX": 0.0,
            "scrollPositionY": 0.0,
        }
        store["."]["vSrn"] = ("long", 1)
        store[app_name]["Iloc"] = app_at
        store["Applications"]["Iloc"] = applications_at
    finally:
        store.close()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", required=True, help="the .app to put in the volume")
    ap.add_argument("--background", required=True, help="the rendered 2x background PNG")
    ap.add_argument("--volume", required=True, help="the volume name Finder shows")
    ap.add_argument("--out", required=True, help="the .dmg to write")
    ap.add_argument("--name", help="the .app's file name; defaults to the --app basename")
    ap.add_argument("--icon-size", type=float, default=149)
    ap.add_argument("--label-size", type=float, default=13)
    ap.add_argument("--app-at", default="160,160")
    ap.add_argument("--applications-at", default="480,160")
    args = ap.parse_args()

    app = pathlib.Path(args.app).resolve()
    background = pathlib.Path(args.background).resolve()
    out = pathlib.Path(args.out).resolve()
    name = args.name or app.name
    app_at = tuple(int(v) for v in args.app_at.split(","))
    applications_at = tuple(int(v) for v in args.applications_at.split(","))

    if not app.is_dir():
        raise SystemExit(f"error: no such bundle: {app}")
    if not background.is_file():
        raise SystemExit(f"error: no such background: {background}")
    out.parent.mkdir(parents=True, exist_ok=True)

    stage = pathlib.Path(tempfile.mkdtemp(prefix="memedock-dmg-"))
    try:
        # The payload, laid out as it should sit in the volume. symlinks=True keeps the
        # bundle's own structure; the app carries a versioned dylib and a signature.
        shutil.copytree(app, stage / name, symlinks=True)
        shutil.copy2(background, stage / ".background.png")
        link = stage / "Applications"
        if not link.exists():
            link.symlink_to("/Applications")
        write_ds_store(
            stage / ".DS_Store", args.volume, name,
            args.icon_size, args.label_size, app_at, applications_at,
        )
        # -srcfolder copies the folder in as it stands, dotfiles, symlink and .DS_Store
        # included, which is why the staging directory is built here and not after.
        result = subprocess.run(
            [
                "hdiutil", "create",
                "-srcfolder", str(stage),
                "-volname", args.volume,
                "-fs", "HFS+",
                "-format", "UDZO",
                "-quiet",
                str(out),
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            sys.stderr.write(result.stderr)
            raise SystemExit(f"error: hdiutil failed with {result.returncode}")
    finally:
        shutil.rmtree(stage, ignore_errors=True)

    print(f"Built {out}  ({out.stat().st_size / 1_000_000:.1f} MB, volume {args.volume!r})")
    return 0


if __name__ == "__main__":
    sys.exit(main())