# MemeDock Web

Read the repository AGENTS.md first. This is a static SvelteKit application embedded
in the Rust server; Axum owns authentication, authorization and business state.

## Boundaries

- `src/routes/`: route metadata and composition. Do not put forms, network state or business operations here.
- `src/lib/features/<feature>/`: API adapters, query options and feature components.
- `src/lib/shared/clients/`: HTTP envelope handling and application QueryClient policy.
- `src/lib/shared/theme/`, `shared/dom/`: shared browser capabilities with cleanup.
- `src/lib/ui/primitives/`: reusable, typed controls; Bits UI owns complex accessibility behavior.
- `src/lib/ui/common/`: shared presentation and the single QueryRoot.
- `src/lib/ui/layout/`: layout composition through snippets; no feature API dependencies.
- `src/lib/generated/`: generated from Rust HTTP DTOs. Never edit or track generated declarations.

## Data and interactions

Use TanStack Query for server state and mutations. Keep form drafts and ephemeral
credentials in their feature, using Svelte runes. Do not mirror query data into a
global store. Clear private caches on logout or session invalidation. Never
persist credentials in query storage, browser storage, routes, logs or screenshots.
Creation mutations must not retry automatically. Forward query cancellation to fetch.

Wrap application content with QueryRoot once. Keep API errors, request IDs,
loading, empty, refetch-error and mutation states explicit. Browser capabilities
must be client-safe and clean up listeners. No `any`, fake content or unavailable navigation.

## Design

Use the Android theme and macOS Design sources as the visual reference. Global
semantic tokens belong in `src/routes/layout.css`; use Tailwind utilities in
components. Keep feature-specific styling local. Do not style every form/button
from the root layout. Maintain Quicksand plus Chinese sans-serif typography,
neutral layered surfaces, restrained blue emphasis, one primary action per view,
and lightweight contextual actions. Preserve keyboard focus, accessible names,
reduced motion, light/dark/system themes and narrow-screen touch targets.

## Verification

Run `pnpm check`, `pnpm build`, and `pnpm test`. Browser tests cover the real Rust
server, authorization, dialogs, focus restoration, account settings and themes.
Inspect desktop/narrow screenshots. Build the release binary through
`node tools/server/build.mjs` from repository root. Report actual checks and
limitations in conversation; do not write progress reports into project files.
