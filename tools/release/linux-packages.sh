#!/usr/bin/env bash
# Build DEB, RPM and AppImage packages from the release staging tree.
# The staging tree is produced by tools/release/linux-bundle.sh.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
version=$(python3 tools/release/version.py --check)
stage="$root/target/release-bundle/MemeDock-$version-linux-x86_64"
distribution="$root/target/distribution/linux"
if [[ ! -d "$stage/usr" ]]; then
    printf '%s\n' "Missing release staging tree: $stage" >&2
    printf '%s\n' "Run tools/release/linux-bundle.sh first." >&2
    exit 1
fi
mkdir -p "$distribution"

build_deb() {
    local work="$root/target/packages/deb"
    local deb_root="$work/MemeDock-$version"
    rm -rf "$work"
    mkdir -p "$deb_root/DEBIAN"
    cp -a "$stage/usr" "$deb_root/"
    local installed_size
    installed_size=$(du -sk "$deb_root/usr" | cut -f 1)
    cat > "$deb_root/DEBIAN/control" <<EOF
Package: memedock
Version: $version
Architecture: amd64
Maintainer: grtsinry43 <grtsinry43@outlook.com>
Section: graphics
Priority: optional
Installed-Size: $installed_size
Homepage: https://github.com/grtsinry43/MemeDock
Depends: libgtk-4-1 (>= 4.22), libadwaita-1-0 (>= 1.9), libc6, libstdc++6
Recommends: xdg-desktop-portal, gnome-keyring
Description: Unified sticker manager for easy importing and quick sharing
 MemeDock organizes stickers in one place, imports them quickly, and makes
 sharing the right image in a chat effortless.
EOF
    local deb="$distribution/MemeDock-$version-linux-x86_64.deb"
    dpkg-deb --root-owner-group --build "$deb_root" "$deb"
    dpkg-deb -c "$deb" | grep -F 'usr/bin/memedock' >/dev/null
}

build_rpm() {
    local work="$root/target/packages/rpm"
    rm -rf "$work"
    mkdir -p "$work"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
    rpmbuild -bb \
        --define "_topdir $work" \
        --define "version $version" \
        --define "stage $stage" \
        "$root/packaging/rpm/memedock.spec"
    local rpm
    rpm=$(find "$work/RPMS" -name 'memedock-*.rpm' -type f -print -quit)
    if [[ -z "$rpm" ]]; then
        printf '%s\n' "rpmbuild did not produce a package" >&2
        exit 1
    fi
    cp "$rpm" "$distribution/MemeDock-$version-linux-x86_64.rpm"
    rpm -qlp "$distribution/MemeDock-$version-linux-x86_64.rpm" | grep -Fx '/usr/bin/memedock' >/dev/null
}

build_appimage() {
    local work="$root/target/packages/appimage"
    local appdir="$work/AppDir"
    local tools="$root/target/appimage/tools"
    local selection="$distribution/MemeDock-$version-linux-x86_64.AppImage"
    rm -rf "$work"
    mkdir -p "$appdir" "$tools"
    cp -a "$stage/usr" "$appdir/"
    rsvg-convert -w 256 -h 256 \
        "$appdir/usr/share/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg" \
        -o "$appdir/com.grtsinry43.memedock.png"
    mkdir -p "$appdir/usr/share/icons/hicolor/256x256/apps"
    cp "$appdir/com.grtsinry43.memedock.png" \
        "$appdir/usr/share/icons/hicolor/256x256/apps/com.grtsinry43.memedock.png"
    ln -sf com.grtsinry43.memedock.png "$appdir/.DirIcon"
    ln -sf usr/share/applications/com.grtsinry43.memedock.desktop \
        "$appdir/com.grtsinry43.memedock.desktop"

    local schema_dir="$appdir/usr/share/glib-2.0/schemas"
    mkdir -p "$schema_dir"
    shopt -s nullglob
    for schema in \
        /usr/share/glib-2.0/schemas/org.gtk.gtk4.Settings.*.gschema.xml \
        /usr/share/glib-2.0/schemas/org.gnome.adw.*.gschema.xml \
        /usr/share/glib-2.0/schemas/org.gnome.desktop.interface.gschema.xml; do
        cp -a "$schema" "$schema_dir/"
    done
    shopt -u nullglob
    glib-compile-schemas "$schema_dir"

    local typelib_dir
    typelib_dir=$(pkg-config --variable=typelibdir gobject-introspection-1.0)
    mkdir -p "$appdir/usr/lib/girepository-1.0"
    local typelib
    for typelib in Gtk-4.0 Adw-1 Gdk-4.0 GdkPixbuf-2.0 Gio-2.0 GLib-2.0 GObject-2.0; do
        if [[ -f "$typelib_dir/$typelib.typelib" ]]; then
            cp -a "$typelib_dir/$typelib.typelib" "$appdir/usr/lib/girepository-1.0/"
        fi
    done

    local loader_dir
    loader_dir=$(pkg-config --variable=gdk_pixbuf_moduledir gdk-pixbuf-2.0 2>/dev/null || true)
    if [[ -n "$loader_dir" && -d "$loader_dir" ]]; then
        local appdir_loaders="$appdir/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders"
        mkdir -p "$appdir_loaders"
        shopt -s nullglob
        local loaders=("$loader_dir"/*.so)
        shopt -u nullglob
        if (( ${#loaders[@]} > 0 )); then
            cp -a "${loaders[@]}" "$appdir_loaders/"
            gdk-pixbuf-query-loaders "$appdir_loaders"/*.so \
                > "$appdir/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache" 2>/dev/null || true
        fi
    fi

    local linuxdeploy="$tools/linuxdeploy-x86_64.AppImage"
    local appimagetool="$tools/appimagetool-x86_64.AppImage"
    if [[ ! -x "$linuxdeploy" ]]; then
        curl -fL --retry 3 -o "$linuxdeploy" \
            "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
        chmod +x "$linuxdeploy"
    fi
    if [[ ! -x "$appimagetool" ]]; then
        curl -fL --retry 3 -o "$appimagetool" \
            "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
        chmod +x "$appimagetool"
    fi

    local gtk_lib adw_lib
    gtk_lib=$(ldconfig -p | awk '/libgtk-4\.so\.1 /{print $NF; exit}')
    adw_lib=$(ldconfig -p | awk '/libadwaita-1\.so\.0 /{print $NF; exit}')
    if [[ -z "$gtk_lib" || -z "$adw_lib" ]]; then
        printf '%s\n' "The GTK4/libadwaita shared libraries were not found." >&2
        exit 1
    fi

    # linuxdeploy is extracted so its bundled patchelf can be replaced with a
    # no-op; the custom AppRun supplies LD_LIBRARY_PATH instead.
    local extract noop
    extract=$(mktemp -d)
    noop=$(mktemp -d)
    printf '#!/bin/sh\nexit 0\n' > "$noop/patchelf"
    chmod +x "$noop/patchelf"
    (cd "$extract" && "$linuxdeploy" --appimage-extract >/dev/null 2>&1)
    cp "$noop/patchelf" "$extract/squashfs-root/usr/bin/patchelf"
    PATH="$noop:$PATH" NO_STRIP=true ARCH=x86_64 \
        "$extract/squashfs-root/AppRun" \
        --appdir "$appdir" \
        --executable "$appdir/usr/bin/memedock" \
        --library "$gtk_lib" --library "$adw_lib" \
        --desktop-file "$appdir/usr/share/applications/com.grtsinry43.memedock.desktop" \
        --icon-file "$appdir/com.grtsinry43.memedock.png"
    rm -rf "$extract" "$noop"
    # linuxdeploy leaves AppRun as a symlink to the executable; remove it first
    # so copying our script cannot overwrite the binary through the link.
    rm -f "$appdir/AppRun"
    cp "$root/packaging/appimage/AppRun" "$appdir/AppRun"
    chmod +x "$appdir/AppRun"
    APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 \
        "$appimagetool" "$appdir" "$selection"
    if [[ ! -s "$selection" ]]; then
        printf '%s\n' "appimagetool did not produce an AppImage" >&2
        exit 1
    fi
    xvfb-run -a "$selection" --appimage-extract-and-run --help >/dev/null
}

build_deb
build_rpm
build_appimage

assets=(
    "MemeDock-$version-linux-x86_64.tar.zst"
    "MemeDock-$version-linux-x86_64.deb"
    "MemeDock-$version-linux-x86_64.rpm"
    "MemeDock-$version-linux-x86_64.AppImage"
)
(cd "$distribution" && sha256sum "${assets[@]}" > SHA256SUMS)
printf '%s\n' "Packaged DEB, RPM and AppImage for MemeDock $version"
