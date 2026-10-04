# Repository Guidelines

## Project Purpose and Current State

MemeDock is a personal, local-first sticker library for Android and Linux: import images, organize with collections and tags, search, and hand images to other applications. Each platform uses its own native UI and its own library instance. Rust shares business logic, not a running process or UI.

This repository currently contains directory scaffolding only. Cargo packages, Android build configuration, dependency versions, application entry points, and CI are not implemented yet. Do not describe directory scaffolding as a working application.

## Project Structure and Dependency Direction

- `crates/domain/`: validated IDs, domain models, changes, and shared business rules. No GTK, Android, SeaORM, Axum, or image-decoding dependencies.
- `crates/storage/`: SeaORM entities, explicit SQLite migrations, queries, transactions, and file-storage primitives. Depends on domain; never on core or UI. Keep ORM entities internal.
- `crates/core/`: application use cases for import, management, search, image processing, exports, archives, task coordination, and change notifications. Depends on domain and storage.
- `crates/ffi/`: UniFFI exports, owned boundary records, and error conversion for Kotlin. Depends on core. Business rules belong in core.
- `apps/android/`: Kotlin, Jetpack Compose, Material 3, ViewModel/StateFlow, and Android system adapters. Calls core through ffi.
- `apps/linux/`: Rust, gtk4-rs/libadwaita, UI state, and Linux system adapters. Calls core directly.
- `tools/`: native-library builds, binding generation and validation, fixture inspection, and release helpers.
- `fixtures/`: legally usable image fixtures and expected metadata. Never add private user images.
- `packaging/android/`, `packaging/arch/`: platform release assets and packaging helpers.
- `docs/`: local product design, architecture discussions, milestone details, and acceptance records. Never track this directory.

Organize business operations by use case inside the established boundaries. Do not create one crate per feature, one repository trait per table, or generic layers without a concrete need. The optional future server belongs in `apps/server/` and must not depend on the full client core or pull in UI/animation/AI dependencies.

## Agent Collaboration Requirements

1. Inspect existing code, project instructions, and available local design documents before proposing changes. If local documents are absent, explain what context is missing; do not invent requirements.
2. For architectural, dependency, API, schema, or build changes, state the strategy, affected files, mechanisms, alternatives, and reasons before implementation. Follow the user's explicitly invoked collaboration workflow and preserve decisions already approved in the session.
3. Check the project's pinned dependency versions and their matching official documentation before using APIs. Do not silently replace a chosen dependency, upgrade toolchains, or copy unrelated projects' technology constraints.
4. Keep changes scoped and reviewable. Surface disagreements and resolve them explicitly; never verbally accept a decision and implement another.
5. Forbid fake implementations, production mock data, TODO-only feature bodies, and error suppression presented as completion. Report exactly what works, what was verified, and what remains unfinished.
6. Ask about expected test coverage before adding tests when following the grt-collaborating workflow. Run relevant existing checks; use real platform verification for system integration.
7. Do not commit, publish, push, or send messages unless authorized. Do not claim unrun checks passed.

## Rust Coding and Ownership

- Use rustfmt, idiomatic naming, cohesive modules, and explicit visibility. Keep implementation details private; expose the smallest useful API.
- Model domain identity with validated newtypes such as `ContentHash` and `CollectionId`. Convert strings at FFI/serialization boundaries rather than passing unchecked strings through business logic.
- Prefer ownership and borrowing that express actual lifetimes. Do not introduce pervasive `Arc<Mutex<_>>`, unnecessary clones, `Box::leak`, or lifetime escape hatches to silence the compiler.
- Use `Result` for fallible operations and stable, meaningful errors at application boundaries. Avoid `unwrap`/`expect` in production input, filesystem, database, and FFI paths. Any panic justified by an invariant must explain that invariant.
- Avoid `unsafe` unless required by a verified integration boundary. Isolate it and document safety invariants, ownership, thread access, and lifetime assumptions. Never use unsafe to bypass an architectural or ownership problem.
- Do not hold locks across `.await` or while invoking foreign callbacks. Keep shared state and critical sections small.
- Keep dependency features deliberate and minimal. Commit application lockfiles when build configuration exists; do not ignore `Cargo.lock`.

## Async Runtime, Tasks, and FFI

- Each library has one owned, bounded execution context. Never create a runtime or thread pool for every FFI call.
- Database work, hashing, image decoding, resizing, and archive work must not block Android's UI thread or GTK's main thread.
- Bound CPU/blocking jobs, queue capacity, and image memory. When using Tokio `spawn_blocking`, apply concurrency limits and cooperative cancellation; aborting a handle does not stop an already running blocking job.
- Use explicit task IDs and cancellation checks. Kotlin coroutine cancellation alone is not proof that Rust work stopped.
- GTK objects stay on the GTK main thread; deliver results through the GLib main context. Android observers update UI state through the appropriate dispatcher.
- FFI transfers owned records, stable error codes, metadata, and artifact paths. Do not expose ORM entities, internal locks, borrowed references, or large per-frame pixel buffers.
- Give library handles, subscriptions, and tasks explicit lifecycle ownership and shutdown behavior. Discard stale search results and bind page cursors to query/sort parameters.

## Persistence and Data Safety

- Client storage is SQLite through SeaORM plus immutable original files. One core instance owns each local library; Android must not maintain a second Room business database.
- Use explicit, reviewable migrations. Apply connection settings consistently: WAL, foreign keys, busy timeout, and the approved durability policy. Use short transactions and bounded connections; serialize writes within core.
- Publish verified original files before committing referencing database records. Metadata changes and their local change-log entries commit together. Thumbnail generation happens after commit and is recoverable.
- Asset and Sticker identities use the full SHA-256 of original bytes. Collections, tags, devices, and operations use UUIDv7. Deduplication means identical bytes, not visual similarity.
- Keep originals immutable. Derived previews, thumbnails, and exports never replace originals. Store hashes or relative identifiers, not cross-device absolute paths.
- Soft deletion preserves identity and recovery information. Do not add automatic original-file garbage collection, recycle-bin purging, or log pruning without a separately approved lifecycle design.
- Keep device-local paths, usage history, caches, credentials, and artifact leases separate from synchronized business state.
- Use parameterized search; never interpolate user text into SQL or FTS syntax. Preserve short Chinese substring queries when changing search indexing.
- Back up a consistent SQLite snapshot, not a live database file without WAL handling. Validate hashes and reject archive paths escaping the destination.

## Image Processing and Platform Boundaries

- Validate actual bytes, format, dimensions, and resource limits; do not trust filename extensions or declared MIME. Bound memory before allocating decoded images.
- Grids show static thumbnails. Animation plays in one detail instance with bounded frame buffering. Do not collect an entire animation into memory.
- Preserve original GIF/WebP animation for original exports. First-frame conversion is explicit. Apply orientation before stripping metadata and composite transparency before JPEG encoding.
- Core owns business rules and export artifacts; platforms own file input, clipboard, sharing, drag-and-drop, credentials, and lifecycle.
- Android reads `content://` inputs through ContentResolver into controlled staging. Never infer a real filesystem path from a content URI. FileProvider exposes only the dedicated share directory.
- Linux exposes explicit image/file copy actions and keeps content providers and artifact leases alive while needed. Do not promise clipboard persistence after process exit on every desktop.
- Sharing launch is a usage attempt, not confirmed message delivery. Do not display fabricated send-success states.

## Scope and Future Extensions

v0.1 completes local import, organization, search, preview, handoff, soft delete/recovery, and portable backup. Sync and AI must not become prerequisites for local use.

Future synchronization must preserve idempotent operation IDs, tombstones, generations, server commit ordering, atomic state/event/receipt updates, and cursor semantics. Do not implement broad whole-record upserts or treat device time as conflict authority. Share protocol/domain code without coupling the server to client image processing.

AI outputs are optional derived data keyed by asset and processing/model version. They must not silently overwrite user titles or tags. Source import adapters feed the normal import flow and never bypass validation or persistence rules.

## Build, Verification, and Reporting

There are currently no buildable Cargo packages, Gradle project, test suite, or CI. The following Rust commands are requirements once the workspace is implemented, not commands that currently pass here:

- `cargo fmt --all -- --check`
- `cargo check --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`

Check relevant feature combinations separately; do not blindly enable incompatible platform features. Verify generated Kotlin bindings match the pinned UniFFI/native library versions. Android release verification includes selected ABIs and 16 KiB page compatibility; Linux acceptance records actual GNOME/KDE, Wayland/X11, GTK/libadwaita, and portal versions.

Prioritize meaningful tests for transactions, deduplication, failure recovery, archive validation, image rules, and later synchronization semantics. Verify clipboard, sharing, URI permissions, and drag-and-drop using real target applications. Report modified files, behavior, limitations, review focus, and actual test/CI results.

## Git and Documentation Policy

- Track this `AGENTS.md`, source, build configuration, migrations, fixtures, and application lockfiles.
- Never track `docs/`, including design documents and milestone details. Keep new design/stage documents in `docs/`; the legacy root design-document path is also ignored.
- Never use `git add -f` to bypass document exclusions. Before staging or committing, inspect `git status`, ignored paths, and the staged diff. If a document is accidentally staged, unstage it without deleting the local copy.
- Keep credentials, signing keys, generated build outputs, and runtime databases out of Git.
- Use Conventional Commits when commits are requested. PR descriptions explain the concrete behavior, rationale, validation, and any migration/configuration impact.

## Official References

- Rust ownership: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html
- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
- Tokio blocking tasks and cancellation: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html

These references inform guidelines; implementation must check documentation matching the dependency versions actually selected by this project.
