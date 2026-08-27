# Typerlude Working Agreements

## Architecture

- Keep Typerlude a single Cargo package with its existing library and binary targets. Do not introduce a Cargo workspace or member crate for internal modularity.
- Treat the codebase as a modular monolith: top-level capabilities, adapters, and infrastructure responsibilities use directory modules whose roots own real types, policy, or orchestration.
- Small shared value, diagnostic, and error leaves may remain flat when a directory would add no responsibility boundary.
- Add modules only for concrete implemented responsibilities. Do not scaffold hypothetical features or extension points.

## Dependency Direction

- CLI and TUI are outer adapters that call the application layer and concrete core capabilities.
- Terminal owns the interactive runtime, event conversion, and terminal lifetime; it may depend on the application and TUI.
- The application layer coordinates state and use cases and must not depend on CLI, TUI, terminal, Ratatui, or Crossterm types.
- Core capabilities must not depend on CLI, TUI, terminal, Ratatui, or Crossterm types. Keep core-to-core dependencies acyclic.
- TUI reads application/core state for presentation; state-changing user actions go through application use cases.
- Headless CLI commands may call the concrete capability they own directly.

## Module Design

- Split by responsibility and dependency boundary, never mechanically by line count.
- Keep tightly coupled invariants together even when the module is long.
- Do not create re-export-only files, empty future modules, compatibility aliases, `common`, `utils`, or `prelude` modules.
- Prefer concrete functions and data flow. Do not add a trait, service interface, factory, DI container, or event bus for a single implementation.
- Reuse existing code and dependencies before adding helpers or abstractions.

## Visibility and Compatibility

- Use the narrowest visibility that works: private, then `pub(super)`, then `pub(crate)`, and `pub` only for deliberate package facades.
- Preserve CLI behavior and output, TUI behavior, terminal restoration, storage schemas, content formats, and bounded/atomic I/O unless a change explicitly requests otherwise.
- Keep structural moves separate from behavior changes and verify the nearest characterization tests after each move.

## Versioning and Releases

- Use stable numeric Semantic Versioning in `MAJOR.MINOR.PATCH` form; releases do not use prerelease or build metadata.
- Choose the bump from all changes since the latest published tag: PATCH for backward-compatible bug fixes, MINOR for backward-compatible functionality, and MAJOR for incompatible changes to public library, CLI, or persisted-format contracts.
- When change types are mixed, use the highest required bump and reset all lower components to zero.
- Never reuse or decrease a published version.
- Use `make release` as the only release entry point; do not hand-edit or publish the synchronized Cargo and npm versions separately.
