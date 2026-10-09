# Repository Guidelines

## Project

MemeDock is a unified sticker manager and image-sharing tool: organize stickers in one place, import them easily, and quickly find and send the right image in chats. Prioritize convenient importing, browsing/searching, and sharing in product and UI decisions. Local-first data handling, shared Rust business logic, and native platform UIs support these workflows.

Develop Rust + Android first; Linux follows. Local import, organization, export, sharing, clipboard, saving, portable backup and recovery are implemented.

Read relevant local designs in `docs/design.md`, `docs/domain-model.md`, and `docs/infrastructure.md`. If absent, use code as evidence and ask about missing requirements; do not invent them.

## Commands

From the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

From `apps/android/`: `./gradlew :app:testDebugUnitTest :app:verifyDebugBridge` builds the APK and verifies Kotlin/native contracts and 16 KB alignment. `:rustBridge:connectedDebugAndroidTest` requires a device/emulator. Gradle orchestrates pinned Cargo/UniFFI tools; generated bindings and libraries stay in build directories. Debug builds arm64/x86_64; release builds arm64; override with `-Pmemedock.abis=arm64-v8a,x86_64`. Generated starter tests do not verify product behavior.

## Boundaries

- `crates/domain`: validated IDs, models, rules, and contracts; no database, runtime, image, or UI dependencies.
- `crates/storage`: SeaORM/SQLite, explicit current schema initialization, queries, and file primitives; depends on domain. ORM entities stay internal.
- `crates/core`: use cases, images, tasks, and notifications; depends on domain/storage.
- `crates/ffi`: UniFFI DTO/error conversion; calls core. Android calls ffi; Linux calls core directly.
- Platforms own input, sharing, clipboard, credentials, and lifecycle. UI never accesses the business database directly.
- Keep originals immutable; publish verified files before committing references. Business changes and their local log commit together. Preserve soft deletion, generations, and explicit recovery.
- Keep one core instance per library. Bound background work and memory; never block UI threads or hold locks across `.await`. FFI passes owned metadata/artifact paths, not internal entities or animation buffers.

## Working Rules

- 0. Guarantee high-quality code; forbid placeholder implementations, fake implementations, or problematic code.
- 1. Follow the explicitly invoked collaboration skill. Propose architectural, dependency, schema, API, and build changes before implementing; preserve approved decisions.
- 2. Keep Rust code idiomatic, use validated types and `Result`, and respect workspace lints. Avoid unnecessary abstractions.
- 3. Before using any library, check its latest version and documentation; leverage search and network access; do not guess or assume usage.
- 4. Verify APIs against the project's pinned versions and matching official documentation; do not silently upgrade dependencies.
- 5. Use `apply_patch` for file creation, edits, and deletion; no ad hoc Python or shell-generated project files.
- 6. Ask about test expectations before adding tests. Run relevant checks and report actual results.
- 7. Report changes, limitations, review focus, and test/CI results directly in conversation, never in project files.
- 8. The initial-release restriction on historical-data compatibility is lifted. Use the first public release (`0.1.0`) as the initial compatibility baseline; intermediate pre-release schema revisions are not separate released versions. Subsequent changes must account for upgrades of existing libraries and supported persisted formats. Preserve originals, transaction rollback, and recovery of interrupted migrations. Do not silently reset incompatible data or assume downgrade support.
- 9. Organize migration and compatibility logic by source release / target application version pair, not by development iteration. Keep all steps for one transition together in a single transition file/module (for example, `v0_1_0_to_v0_2_0.rs`); extend that file with ordered functions or sections as development continues. Do not create a separate timestamped/numbered migration file for every intermediate schema edit, feature, or commit within the same unreleased target version. Preserve the existing layer/platform boundaries.
- 10. Keep completed, released transitions stable; new feature/schema changes belong to the next version transition. Support upgrades across multiple releases by applying the completed transitions in order, rather than duplicating steps for every possible version pair. Keep fresh-library initialization aligned with the latest schema; specify supported source versions and failure/recovery behavior when proposing each migration.

## Git

- Track `AGENTS.md`, source, schema, build configuration, and lockfiles. Never track `docs/`, credentials, signing keys, runtime data, or build outputs; never force-add ignored documents.
- Inspect status and staged changes before committing. Commit/push/publish only when requested; use Conventional Commits.
