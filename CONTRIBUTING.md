# Contributing

Thanks for contributing to `objsds`. The project values focused, well-tested
changes that preserve its small protocol and explicit consistency guarantees.
These conventions keep changes straightforward to review and revert.

## Set up the repository with mise

This repository uses [mise](https://mise.jdx.dev/) as the source of truth for
development tools and common tasks. Start by installing the pinned toolchain:

```console
mise install
```

Using mise is important: it gives contributors and CI compatible versions of
Rust, hk, cargo-deny, Pitchfork, and the other tools used to validate the
workspace. Prefer `mise run <task>` over invoking a locally installed
alternative so that results remain reproducible.

Before opening a pull request, run:

```console
mise run ci
```

This checks formatting, Clippy, tests, dependency policy, and package contents.
Changes involving the S3 adapter should also run the RustFS end-to-end suite:

```console
mise run test:e2e
```

Performance-sensitive changes can be evaluated with `mise run test:perf`.
See `README.md` for the test's scope and configuration.

## What a good pull request looks like

- **Address one concern.** Keep features, fixes, and unrelated refactors in
  separate pull requests when practical. Focused changes are easier to review,
  revert, and reason about.
- **Use an accurate Conventional Commit title.** Prefix the pull request title
  with `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, or another
  suitable type. Maintainers may squash-merge, making the title the resulting
  commit message.
- **Preserve compatibility by default.** Prefer optional additions and
  conservative defaults. Call out changes to public APIs, object formats,
  storage semantics, or minimum supported Rust versions.
- **Test observable behavior.** Add or update tests for behavior changes. For
  storage adapters, use the shared backend contract where possible and include
  an end-to-end test when behavior depends on a real backend.
- **Update documentation.** Document user-facing APIs, configuration, storage
  requirements, and behavioral guarantees in the same pull request.
- **Describe validation.** State which `mise` tasks you ran and disclose any
  relevant checks you could not run.

## Style

Rust formatting and linting are enforced by `cargo fmt` and Clippy through the
mise tasks. Match the surrounding code, keep public APIs and non-obvious
invariants documented, and avoid reformatting files unrelated to your change.
Run `mise run fmt` and `mise run lint` while iterating, then `mise run ci`
before submission.
