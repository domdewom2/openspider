# CI and release

Workflows are executable policy. Check the linked YAML for exact triggers, inputs and path filters; a green documentation-only commit need not have run every suite. [M2 K2-67](../../plan/M2.md) explains consolidation and reduced duplicate runs.

## Main platform coverage

[ci.yml](../../../.github/workflows/ci.yml) normally selects macOS arm64 (`macos-14`) and Windows x64 (`windows-latest`). macOS Intel (`macos-15-intel`) joins scheduled runs, `v*` tags and manual runs unless deselected. Separate Ubuntu jobs run rustfmt and cargo-deny (licenses, bans, sources, advisories).

Each platform job installs Node 22 dependencies, checks frontend types/tests/static rules/build, runs locked Clippy and Rust tests, checks the Tauri build, creates a fixture crawl and runs small load tests. Test compilation is separate from execution. The current Rust execution timeout is **50 minutes**, although the plan's A1 target remains under 15 minutes (K-81/K2-64); do not mistake the target for the current YAML setting. The cross-platform load stage uses uploaded CLI binaries to reload the other platforms' crawl files and compare exports byte for byte. These details are in [ci.yml](../../../.github/workflows/ci.yml).

Linux server support is a separate [linux-core.yml](../../../.github/workflows/linux-core.yml) job using [linux-core.sh](../../../scripts/ci/linux-core.sh): no desktop, display or D-Bus, memory secrets, headless Chromium, release CLI and golden/read-API smoke checks. The [getting-started commands](getting-started.md) mirror this path.

## Workflow map

| Workflow | Checks / output |
|---|---|
| [ci.yml](../../../.github/workflows/ci.yml) | Main macOS/Windows matrix, frontend/Rust/Tauri, crawl interchange, small load tests and M1 coverage |
| [linux-core.yml](../../../.github/workflows/linux-core.yml) | Ubuntu core, CLI, MCP-STDIO and independent SQLite read API smoke |
| [cloud-setup.yml](../../../.github/workflows/cloud-setup.yml) | Linux setup, repeated setup/idempotence and environment smoke |
| [neutral.yml](../../../.github/workflows/neutral.yml) | Repository neutrality scan |
| [spec-coverage-m2.yml](../../../.github/workflows/spec-coverage-m2.yml) | Actual Rust, Vitest and E2E JUnit evidence; M2 coverage/appendix B, optional strict milestone gate |
| [parity.yml](../../../.github/workflows/parity.yml) | Fixture export comparison on macOS; checks availability of checked-in comparison inputs |
| [ui-e2e.yml](../../../.github/workflows/ui-e2e.yml) | E2E-A: macOS WebKit, Windows Chromium, Linux both engines |
| [ui-e2e-b.yml](../../../.github/workflows/ui-e2e-b.yml) | Native app/WebView checks and evidence on macOS/Windows; nightly/manual/reusable |
| [integrations.yml](../../../.github/workflows/integrations.yml) | Integration asset format/schema/catalog checks, launcher contracts and installed integration evidence |
| [resources-nightly.yml](../../../.github/workflows/resources-nightly.yml) | Linux resource pause/resume and one-million-URL RSS check |
| [scale-nightly.yml](../../../.github/workflows/scale-nightly.yml) | macOS 100k/250k measurements and storage-growth comparison |
| [perf-diag.yml](../../../.github/workflows/perf-diag.yml) | Manual platform diagnostic probes, crawl variants and writer benchmark |
| [spike-grid.yml](../../../.github/workflows/spike-grid.yml) | Real WebView grid evidence; indicative platform measurements |
| [spike-platform.yml](../../../.github/workflows/spike-platform.yml) | Reuses native harness for menu and visibility checks |
| [release-macos.yml](../../../.github/workflows/release-macos.yml) | Signed/notarized macOS application and DMG artifacts |

For dependency policy consult [deny.toml](../../../deny.toml) and the frontend [license checker](../../../apps/desktop/scripts/check-licenses.mjs). The neutrality workflow runs [check-neutral.sh](../../../scripts/check-neutral.sh). It scans tracked text, so stage new documentation before relying on its result.

## macOS signing and release artifacts

The [release workflow](../../../.github/workflows/release-macos.yml) accepts manual architecture selection (`universal`, `aarch64`, `x86_64`); `v*` tags select universal. It builds with Tauri, signs with Hardened Runtime, notarizes and staples the app, then separately signs/notarizes/staples the DMG. [verify-macos.sh](../../../scripts/release/verify-macos.sh) checks the signature, runtime, Gatekeeper assessment and stapled ticket. The workflow uploads a DMG and verification logs as Actions artifacts retained for 14 days; it does **not** publish a GitHub Release.

Signing configuration is managed by maintainers through Actions settings; no values or certificate material belong in documentation or commits. The workflow skips building with a warning and successful status when required configuration is missing. A green workflow alone therefore does not prove a signed artifact exists. See the [release operations guide](../../release/macos-signing.md) for maintained operational instructions.

Local artifact verification, using the [verification script](../../../scripts/release/verify-macos.sh):

```sh
scripts/release/verify-macos.sh /Applications/OpenSpider.app
```

The [roadmap](../../../README.md) places broader installer/auto-update delivery in M3. TODO(handbook): Document Windows signing and public distribution when a corresponding release workflow and maintained runbook exist; the current workflow set provides only the macOS signing pipeline.
