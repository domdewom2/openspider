# Introduction

OpenSpider is an Apache-2.0 website crawler and technical SEO auditor with a Rust core, a desktop application, a CLI and an MCP server. Start with the [project README](../../../README.md) for user-facing examples and [license](../../../LICENSE) for terms.

This handbook is for contributors and project agents. Read [getting started](getting-started.md), then the architecture chapter for the layer you will change. Read [testing](testing.md) and [contributing process](contributing-process.md) before preparing a contribution. The [glossary](glossary.md) supplies English and German terms.

## Repository map

| Path | Start here for |
|---|---|
| [Cargo.toml](../../../Cargo.toml) | Rust workspace membership and shared dependencies |
| [crates/](../../../crates) | Core, service and developer-tool implementations |
| [apps/desktop/](../../../apps/desktop) | React frontend and Tauri host |
| [docs/spec/README.md](../../spec/README.md) | Requirements, IDs and milestone scope |
| [docs/ARCHITECTURE.md](../../ARCHITECTURE.md) | Architecture principles and layer boundaries |
| [docs/plan/M1.md](../../plan/M1.md), [M2.md](../../plan/M2.md) | Work packages, integration rules and decision tables |
| [tests/fixtures/](../../../tests/fixtures), [tests/golden/](../../../tests/golden) | Local inputs and expected outputs |
| [scripts/ci/](../../../scripts/ci), [.github/workflows/](../../../.github/workflows) | Executable checks and platform coverage |
| [docs/release/](../../release) | Release operations |
| [integrations/](../../../integrations) | Agent-facing integration assets |

## Reading sources

The handbook is a navigation layer. Requirements live in the [specification](../../spec/README.md); package scope and decisions live in the [plans](../../plan/M2.md); implementation details are checked against linked code. A plan target is not proof that a feature has shipped. Older comments can describe a scaffold that has since been implemented: follow the current function body and tests. Unresolved documentation gaps are marked `TODO(handbook):`.

Clean-room work uses only the repository specification as the implementation brief. Do not inspect the reference application, bytecode or resources, copy its text, or name it in contributions. See [repository rules](../../../CLAUDE.md) and the [neutrality check](../../../scripts/check-neutral.sh).

## Building this handbook

From the repository root:

```sh
~/.cargo/bin/mdbook build docs/handbook
```

The [book configuration](../book.toml) sets the future site prefix to `/docs/`; [book output is ignored](../.gitignore). Chapter links are relative. Source links intentionally resolve within a repository checkout.

TODO(handbook): Define website publishing and map repository source links to a browsable source tree before hosting at openspider.de/docs. This package supplies the book, without a deployment workflow.
