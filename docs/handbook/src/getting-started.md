# Getting started

Run commands from the repository root. Use the checked-in [Rust toolchain](../../../rust-toolchain.toml) (`stable`, rustfmt, clippy); the [workspace](../../../Cargo.toml) currently requires Rust 1.90 or newer. Use Node 22 as in [CI](../../../.github/workflows/ci.yml); the [desktop manifest](../../../apps/desktop/package.json) requires Node >=22. `npm ci` installs the locked npm workspace dependencies, as in CI.

## macOS and Windows

The desktop targets are macOS 13+ on Apple Silicon/Intel and Windows 10 1809+/11 x64 with WebView2 Evergreen; see [architecture platform requirements](../../ARCHITECTURE.md) and [Tauri configuration](../../../apps/desktop/src-tauri/tauri.conf.json). CI uses macOS and Windows runners and Bash for its command steps, with the Windows MSVC target listed in [dependency policy](../../../deny.toml).

TODO(handbook): Add a repository-maintained fresh-machine checklist for the macOS SDK/Xcode command-line tools and Windows C++ build tools. Current workflows assume runner toolchains; they do not document a complete manual installation.

Build the CLI using the [README quick start](../../../README.md):

```sh
cargo build --release -p os-cli
```

The binary is `target/release/openspider` (`openspider.exe` on Windows; see the binary handling in [CI](../../../.github/workflows/ci.yml)). The [desktop scripts](../../../apps/desktop/package.json) and [Tauri build configuration](../../../apps/desktop/src-tauri/tauri.conf.json) provide these development commands:

```sh
npm ci
npm run -w apps/desktop tauri -- dev
```

Tauri's `beforeDevCommand` starts Vite. To run just the frontend (the IPC client can use its mock transport), use `npm run -w apps/desktop dev`; see [IPC transport selection](../../../apps/desktop/src/ipc/client.ts). For the exact CI desktop build check:

```sh
npm run -w apps/desktop tauri -- build --debug --no-bundle
```

## Before handing in a change

These are the exact checks from [CLAUDE.md, “Vor Abgabe”](../../../CLAUDE.md):

```sh
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
npm run -w apps/desktop typecheck && npm run -w apps/desktop test
```

The [CI workflow](../../../.github/workflows/ci.yml) additionally uses locked dependencies, separates test compilation from execution, and checks static frontend rules and production output:

```sh
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked --no-run
cargo test --workspace --locked
npm run -w apps/desktop check:static
npm run -w apps/desktop build
```

These commands describe normal contribution checks. Follow any narrower package instructions, including restrictions during shared-machine measurements; see [resource isolation](../../plan/M2.md) (C11).

## Linux core, CLI and MCP

Linux server CI intentionally excludes `openspider-desktop` and uses a memory secret store without D-Bus or a display. Use the checked-in [Linux wrapper](../../../scripts/ci/linux-core.sh) and [workflow](../../../.github/workflows/linux-core.yml):

```sh
scripts/ci/linux-core.sh fmt
scripts/ci/linux-core.sh clippy
scripts/ci/linux-core.sh chromium
scripts/ci/linux-core.sh test
scripts/ci/linux-core.sh release
scripts/ci/linux-core.sh golden
```

For a complete run with Chromium passed between steps, use `scripts/ci/linux-core.sh all`. It runs the same sequence, copies the release CLI to `target/linux-core/openspider`, and runs the CLI golden smoke. The script contains the exact underlying Cargo commands and the [reasoned skip list](../../../scripts/ci/linux-core-skips.txt).

`chromium` prefers `OPENSPIDER_CHROMIUM`, then a system Chrome/Chromium, then Playwright Chromium. The fallback needs installed workspace Playwright or `PLAYWRIGHT_VERSION` (the Linux workflow supplies it). The step writes the chosen path to `target/linux-core/chromium.path` and, in Actions, `GITHUB_ENV`. Separate local script invocations cannot export variables into the calling shell. Before subsequent rendering tests, explicitly import that result:

```sh
scripts/ci/linux-core.sh chromium
export OPENSPIDER_CHROMIUM="$(cat target/linux-core/chromium.path)"
scripts/ci/linux-core.sh test
```

This shell import mirrors `run_all` in the [wrapper](../../../scripts/ci/linux-core.sh). The smoke itself uses `--no-sandbox`; passing it does not prove that sandboxed rendering tests can start Chromium.

### Ubuntu 26.04 notes (30 October 2026)

The following package workaround was reported by the orchestrator for this handbook: `npx playwright install-deps` failed on Ubuntu 26.04. Install these packages manually before browser use:

```sh
sudo apt-get install libnss3 libnspr4 libatk1.0-0t64 libatk-bridge2.0-0t64 libcups2t64 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 libxrandr2 libgbm1 libasound2t64 libpango-1.0-0 libcairo2
```

TODO(handbook): Add a checked-in Ubuntu 26.04 setup record confirming this package list; it is an operator report, not yet an automated repo recipe. The existing [cloud setup](../../../scripts/cloud/setup.sh) remains the source for the general Linux setup.

For Chromium sandbox startup, the [UI E2E workflow](../../../.github/workflows/ui-e2e.yml) explicitly permits unprivileged user namespaces on Linux:

```sh
sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
```

The orchestrator reports the same requirement on Ubuntu 26.04. This changes a host kernel setting; apply it to the development/test host when required. Select and verify Chromium through `scripts/ci/linux-core.sh chromium` as above.

## Start a crawl

After building, invoke the CLI by its path, or put its directory on `PATH`. With `openspider` on `PATH`, the [README](../../../README.md) example is:

```sh
openspider --crawl https://example.com --headless \
  --output-folder ./out --export-tabs "Internal:All" --save-crawl
```

For MCP startup and options, follow the [CLI help implementation](../../../crates/os-cli/src/help.rs) and [MCP CLI](../../../crates/os-mcp/src/cli.rs). For browser tests and load tests, continue with [testing](testing.md).
