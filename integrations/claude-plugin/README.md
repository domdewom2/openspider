# OpenSpider for Claude Code

This plugin bundles an OpenSpider skill, three audit commands and a local STDIO MCP
connection. Requirements: Claude Code with plugin support, **Node.js 22+** (`node` in
PATH), and an installed OpenSpider CLI. The Node launcher adds useful installation
errors while keeping stdout reserved for MCP. No background HTTP server is required.

## Install from the repository marketplace (macOS and Windows)

In a terminal at the OpenSpider checkout root, install the CLI from source:

```sh
cargo install --path crates/os-cli --locked
openspider --version
node --version
```

Rust/Cargo is needed for this source installation only. Cargo puts the binary in its
configured bin directory. Ensure that directory is on PATH (usually `$HOME/.cargo/bin`
on macOS, `$env:USERPROFILE\.cargo\bin` in PowerShell), then restart Claude Code.
For an already installed CLI, no source build is necessary. The executable is
`openspider` on macOS and `openspider.exe` on Windows. `OPENSPIDER_BINARY` may instead
point to the absolute executable path; use no surrounding quote characters in the
variable value. Paths with spaces work. Never set it to a shell command plus flags.

Inside Claude Code, from the checkout root:

```text
/plugin marketplace add ./integrations
/plugin install openspider@openspider-marketplace
```

Alternatively, use `claude plugin marketplace add ./integrations` and
`claude plugin install openspider@openspider-marketplace` in a terminal. These commands
are the same on both platforms; a checkout elsewhere can be added by its absolute
`integrations` directory. Restart the session or reload plugins, then check `/mcp`.
The marketplace is inside the repository's `integrations/` directory; adding the
repository root as a marketplace would not find this manifest.

## Commands

Claude Code namespaces plugin commands. The files implementing the spec's
`/openspider-audit`, `/openspider-compare` and `/openspider-issues` appear as:

```text
/openspider:openspider-audit https://example.com/
/openspider:openspider-compare <previous-crawl-id> <current-crawl-id>
/openspider:openspider-issues <crawl-id> de
```

The client may display a different qualified form; use its command picker. For the
unqualified spellings, you can create project commands in `.claude/commands/` using
the three `commands/*.md` files as templates. Replace `${CLAUDE_PLUGIN_ROOT}` in those
copies with the actual plugin directory: project commands have no plugin root variable.
The plugin does not write those files automatically. The `openspider` skill can also
be selected directly.

Start with saved crawls. The server invokes **`openspider mcp`** without additional
class grants; by default only `read` is enabled. To allow a requested new crawl, the
operator can enable `classes = ["read", "crawl"]` in OpenSpider's `mcp.toml` and restart
the MCP connection. Export additionally needs `write`. Paid operations need `cost`;
the bundled workflows do not require it. Existing `[tools]` and domain restrictions
still apply. The plugin never edits these permissions. Keep private-target access
disabled for public-site audits.

OpenSpider reads its standard crawl folder, or `<OPENSPIDER_HOME>/crawls` when that
variable is set. Keep the same home for CLI crawls and the MCP process. To print an
equivalent direct client configuration, run `openspider mcp --print-config claude-code`.
You can use that to connect the binary directly if you do not want the Node launcher.

If the binary is absent the launch error includes the source-install command, PATH
and `OPENSPIDER_BINARY` instructions. If `node` itself is absent, install Node.js 22+
or use the direct binary configuration. In either case restart the client after
changing environment variables. No audit has run until tool calls succeed.

Snapshot comparisons use the installed server's available read tools. This version
does not assume a dedicated comparison tool. It reports count/URL evidence and the
limits of differing crawl coverage and configuration.

## Format sources

- [Claude Code plugin manifests](https://code.claude.com/docs/en/plugins-reference)
- [Local marketplace installation](https://code.claude.com/docs/en/plugin-marketplaces)
- [Agent Skills format](https://agentskills.io/specification)

The repo's `integrations/scripts/validate.mjs` checks the supported manifest schema
subset, local skill links and the tool catalog against `os-mcp::tools::all()`.
