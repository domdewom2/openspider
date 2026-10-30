# Contributing to OpenSpider

Read the [contributor handbook](docs/handbook/src/introduction.md), [specification](docs/spec/README.md) and [repository rules](CLAUDE.md) before changing behavior. The [contributing process](docs/handbook/src/contributing-process.md) explains packages, ownership and review evidence.

OpenSpider is licensed under [Apache-2.0](LICENSE). The name/logo have separate terms in the [README](README.md). Dependency licenses must pass [deny.toml](deny.toml): Apache-2.0, Apache-2.0 WITH LLVM-exception, MIT, MIT-0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, BSL-1.0, Unicode-3.0 and MPL-2.0. Check npm dependencies with the [same-policy checker](apps/desktop/scripts/check-licenses.mjs); coordinate policy changes with the designated owner.

Follow the clean-room rule: implement from `docs/spec`; never inspect the reference product, its bytecode or resources, copy its text, or name it. Report ambiguous requirements instead of guessing. Implemented features need fixture-backed tests tagged with their Spec-IDs ([CLAUDE.md](CLAUDE.md)).

Keep commits small with a scope prefix such as `fetch:`, `parse:`, `ui:` or `docs:`. Stage only explicit owned paths. Respect the assigned package scope, existing work and platform-neutral paths ([repository rules](CLAUDE.md)).

Before a PR, the exact “Vor Abgabe” commands from [CLAUDE.md](CLAUDE.md) are:

```sh
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
npm run -w apps/desktop typecheck && npm run -w apps/desktop test
```

The [getting-started chapter](docs/handbook/src/getting-started.md) adds CI's locked/static/build checks and the Linux core-only commands. Follow explicit package restrictions on shared machines. Include check results and unresolved Spec-IDs in the PR; never claim checks you did not run. Run the [neutrality check](scripts/check-neutral.sh) after staging new text:

```sh
bash scripts/check-neutral.sh
```
