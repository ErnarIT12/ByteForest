# Contributing to ByteForest

Thanks for your interest in ByteForest. Bug reports, ideas and pull requests are welcome. By participating you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

Requirements:

- Windows 10/11 x64
- Rust stable with the MSVC toolchain (the repository pins `stable` in `rust-toolchain.toml`)
- Visual Studio Build Tools (MSVC x64 and a Windows SDK)
- Node 24 or newer

Get the code and install frontend dependencies:

```powershell
git clone https://github.com/OWNER/byteforest.git
cd byteforest
npm --prefix app ci
```

Run the app in development mode:

```powershell
npm --prefix app run tauri dev
```

Run it from an elevated terminal if you want to exercise the MFT scanner.

The `app` crate embeds the built frontend, so build it once before running cargo commands on the whole workspace:

```powershell
npm --prefix app run build
cargo test --workspace
```

### Admin tests

Some integration tests compare the MFT scanner with the directory walker on a real volume. They need administrator rights and are ignored by default. Run them from an elevated shell:

```powershell
$env:BF_ADMIN_TESTS = "1"
cargo test -p bf-scan --release --test mft_vs_walk -- --include-ignored --nocapture
```

The drive defaults to the system drive; set `BF_TEST_DRIVE` (a letter such as `E`) to use another one. CI runs the same command on a Windows runner.

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary` in the imperative mood. Common types are `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci` and `chore`. Examples:

```text
feat(scan): add record-by-record MFT fallback
fix(ui): keep tree selection after zoom
docs: explain admin tests
```

## Pull request checklist

CI runs the same checks. Please run them locally before you open a pull request:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check

npm --prefix app run typecheck
npm --prefix app run lint
npm --prefix app test
```

Also:

- Add or update tests for behavior you change. For logic in `bf-core` and `bf-ntfs`, write the test first.
- Keep parsing code free of panics: anything read from disk is untrusted input.
- Keep `bf-core` and `bf-ntfs` free of Windows API dependencies.
- User-visible strings live in `app/src/strings.ts`.
- Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
- Keep pull requests focused: one change per pull request.

## Reporting bugs and security issues

Use the issue templates for bugs and feature requests. For security problems, follow [SECURITY.md](SECURITY.md) and do not open a public issue.

## License

By contributing you agree that your contribution is dual licensed under MIT OR Apache-2.0, the same as the project, without any additional terms or conditions.
