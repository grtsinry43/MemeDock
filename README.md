<div align="center">
  <img src="apps/linux/data/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg" width="96" height="96" alt="MemeDock Logo" />
  <h1>MemeDock</h1>
  <p><em>A quiet, local-first home for your stickers.</em></p>

  <p>
    <a href="https://github.com/grtsinry43/MemeDock/releases"><img src="https://img.shields.io/github/v/release/grtsinry43/MemeDock?style=flat-square&color=1F6FEB" alt="Release" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue?style=flat-square" alt="License: MIT" /></a>
    <img src="https://img.shields.io/badge/Rust-1.94.1-orange?style=flat-square&logo=rust" alt="Rust 1.94.1" />
    <img src="https://img.shields.io/badge/Platforms-macOS%20%7C%20Linux%20%7C%20Android%20%7C%20Windows%20(Preview)-2ea44f?style=flat-square" alt="Platforms" />
  </p>

  <p>
    <strong>English</strong> · <a href="README.zh-CN.md">简体中文</a>
  </p>
</div>

MemeDock is a cross-platform sticker manager built around a shared Rust core and native system interfaces. It brings scattered images and animations into a single local library—making it simple to collect, organize, and share across everyday conversations.

<div align="center">
  <img src="assets/screenshots/hero-desktop.webp" width="90%" alt="MemeDock Overview" />
</div>

## Features

- **Collect from Anywhere** — Bring in images through drag-and-drop, clipboard paste, file selection, or the Android share sheet. Browse and import **Telegram sticker packs** directly by name or link, with `.tgs` and `.webm` stickers converted into transparent animated WebP automatically.
- **Thoughtful Organization** — Arrange stickers with collections, tags, and stars. Reorder items freely, batch-edit selections, search by title, note, filename, or tag, and let frequently used stickers surface naturally.
- **Adaptable Sharing** — Original files remain untouched. When copying, dragging, or sharing, MemeDock prepares the right format on demand—**Original**, **Compatible PNG** (`1024 px`), **White Background PNG**, or **Small JPEG** (`512 px`)—while preserving animation or extracting the first frame when a still image is needed.
- **Local & Enduring** — Everything lives on your device in a content-addressed (`SHA-256`) library with automatic deduplication and a recoverable Trash. Back up or migrate the entire library as a single archive, with content preview, non-destructive merging, and snapshot-protected replacement.
- **Truly Native** — One memory-safe Rust engine (`#![forbid(unsafe_code)]`) paired with native interfaces for every platform: **SwiftUI & AppKit** on macOS, **GTK4 & Libadwaita** on Linux, **Jetpack Compose** on Android, and **WinUI 3** on Windows *(preview)*. Supports instant in-app switching between English and Chinese.

## Screenshots

| Telegram Import | Batch Organization |
| :---: | :---: |
| <img src="assets/screenshots/telegram-import.webp" alt="Telegram Import" /> | <img src="assets/screenshots/batch-organize.webp" alt="Batch Organization" /> |
| **Output Presets** | **Android Client** |
| <img src="assets/screenshots/output-backup-settings.webp" alt="Output Presets" /> | <img src="assets/screenshots/android-overview.webp" alt="MemeDock on Android" /> |

## Installation

Prebuilt packages and `SHA256SUMS` manifests are available on [GitHub Releases](https://github.com/grtsinry43/MemeDock/releases).

- **macOS (`arm64`)**: Download `MemeDock-<version>-macos-arm64.dmg` (or `.zip`) and drag `MemeDock.app` to Applications.
- **Android (`arm64-v8a`)**: Download and install `MemeDock-<version>-android-arm64.apk`.
- **Arch Linux (`x86_64`)**: Build from `packaging/arch/memedock-bin` (prebuilt) or `packaging/arch/memedock-git` (from source) via `makepkg -si`.
- **Linux Bundle (`x86_64`)**: Requires `gtk4 >= 4.22` and `libadwaita >= 1.9`:
  ```sh
  sudo tar --zstd -xf MemeDock-*-linux-x86_64.tar.zst -C /
  ```

## Architecture & Development

```text
crates/domain     Validated identities, models, export rules, and contracts
crates/storage    SQLite (SeaORM), atomic file staging, and content-addressed blobs
crates/core       Use cases, task scheduling, media pipelines, Telegram, and backups
crates/ffi        UniFFI bindings and event streams for Swift, Kotlin, and C#
apps/*            Native clients for android, linux, macos, and windows (preview)
```

```sh
# Workspace verification
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

# Platform builds
cargo build --release --locked -p memedock-linux                        # Linux
bash tools/macos/build.sh release                                       # macOS (.app)
(cd apps/android && ./gradlew :app:testDebugUnitTest :app:verifyDebugBridge) # Android
pwsh tools/windows/build.ps1                                            # Windows (Preview)
```

## License

Released under the [MIT License](LICENSE). The bundled Quicksand typeface is licensed under the [SIL Open Font License 1.1](apps/android/app/src/main/assets/font_licenses/quicksand-OFL.txt).
