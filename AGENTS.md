# Repository Guidelines

## Project

MemeDock is a local-first sticker library with shared Rust business logic and native platform UIs. Develop Rust + Android first; Linux follows. Currently only the domain crate and Android starter project are implemented.

Read relevant local designs in `docs/design.md`, `docs/domain-model.md`, and `docs/infrastructure.md`. If absent, use code as evidence and ask about missing requirements; do not invent them.

## Commands

From the repository root:

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

From `apps/android/`: `./gradlew :app:testDebugUnitTest :app:assembleDebug`. Device tests require a device/emulator; generated starter tests do not verify product behavior.

## Boundaries

- `crates/domain`: validated IDs, models, rules, and contracts; no database, runtime, image, or UI dependencies.
- `crates/storage`: SeaORM/SQLite, explicit migrations, queries, and file primitives; depends on domain. ORM entities stay internal.
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

## Git

- Track `AGENTS.md`, source, migrations, build configuration, and lockfiles. Never track `docs/`, credentials, signing keys, runtime data, or build outputs; never force-add ignored documents.
- Inspect status and staged changes before committing. Commit/push/publish only when requested; use Conventional Commits.
