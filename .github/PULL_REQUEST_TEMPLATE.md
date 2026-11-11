## Summary

<!-- What does this change and why? Link related issues, for example "Closes #123". -->

## Checklist

- [ ] Commits follow Conventional Commits
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo deny check` passes
- [ ] `npm --prefix app run typecheck`, `lint` and `test` pass
- [ ] Tests added or updated for changed behavior
- [ ] `CHANGELOG.md` updated under `[Unreleased]` for user-visible changes
- [ ] Admin tests run if scanners changed (`BF_ADMIN_TESTS=1`)
