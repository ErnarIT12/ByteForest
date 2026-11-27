# Repository Guidelines

## Project Structure

ByteForest is a Rust disk analyzer with a React and TypeScript desktop UI. `app/src/` contains React components, state, and Vitest tests. `app/src-tauri/` contains Tauri commands and platform integration. In `crates/`, `bf-core` builds the file tree and treemap, `bf-ntfs` parses NTFS metadata, `bf-scan` handles filesystem traversal, and `bf-cli` exposes the command line. Rust integration tests live under each crate’s `tests/`. App icons live in `app/src-tauri/icons/`, project artwork in `assets/`, and screenshots in `docs/screenshots/`.

## Build and Development

Run commands from the repository root. Use Node 24 or newer and the Rust toolchain in `rust-toolchain.toml`. Windows needs MSVC and the Windows SDK. macOS needs Xcode Command Line Tools.

- `npm --prefix app ci` installs locked frontend dependencies.
- `npm --prefix app run tauri dev` starts the desktop app for development.
- `npm --prefix app run build` type-checks and bundles the frontend. Run it before workspace-wide Cargo checks because Tauri embeds the output.
- `npm --prefix app run tauri build` builds the desktop package for the host platform.

## Coding Style

Use UTF-8, LF, and spaces. Indent Rust with four spaces and TypeScript, JSON, and YAML with two. Use `snake_case` for Rust functions, `PascalCase` for React components, and `camelCase` for TypeScript functions. Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and the app’s `typecheck` and `lint` scripts. Keep `bf-core` and `bf-ntfs` independent of Windows APIs. Handle malformed disk data without panics.

## Testing

Run `cargo test --workspace`, `npm --prefix app test`, and `cargo deny check`. Name Rust tests after the behavior they verify and colocate frontend tests as `*.test.ts`. Add or update tests for changed behavior. For scanner changes, run the Windows [administrator tests](CONTRIBUTING.md#admin-tests) from an elevated shell.

## Commits and Pull Requests

Use the repository’s documented Conventional Commits style, such as `fix(scan): handle unreadable folders`. Keep pull requests focused. Explain what changed and why, link relevant issues with `Closes #123`, and report checks run. Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes. Report security issues privately via [SECURITY.md](SECURITY.md).

## Performance & Simplicity Guardrails

- Keep React components concise (~300 lines max); extract state, effects, and IPC subscriptions into custom hooks (`app/src/hooks/`) and pure logic into `*.ts` model modules.
- Never call `setState` synchronously during render; reset state via React `key`, derived values, or event handlers and effects.
- Subscribe to Zustand stores with granular selectors (primitives or minimal slices); isolate fast-changing subscriptions (`selectedId`, `focusId`, `idSet`) in leaf components rather than top-level shells.
- Paginate or virtualize lists that can exceed 100 rows (for example via `@tanstack/react-virtual` or incremental "Show more" batches).

## Branch, Worktree, and Merge Workflow

- Keep `main` clean at all times; never commit directly to `main`.
- For every new feature or fix, create a dedicated `git worktree` from up-to-date `main` on a topic branch (`feat/...`, `fix/...`, etc.) and do all work strictly inside that worktree.
- Before pushing, run the full local verification suite: `npm --prefix app run typecheck`, `npm --prefix app run lint`, `npm --prefix app test`, `npm --prefix app run build`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo deny check`.
- Push the branch and open a Pull Request via `gh pr create` with a clear summary of changes and checks run.
- Watch CI checks (`gh pr checks --watch`); if any check fails or reports issues, fix them on the branch and push until all required checks pass.
- Before merging, self-review the PR diff, then merge into `main`, verify that `main` is clean and green, and remove the temporary worktree and merged branch.
