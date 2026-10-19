# OpenSpider integrations

- `skills/openspider/`: portable Agent Skills entrypoint and references (AGENT-041).
- `claude-plugin/`: self-contained Claude Code plugin; see its [installation guide](claude-plugin/README.md) (AGENT-042).
- `.claude-plugin/marketplace.json`: local marketplace; add **this directory**, not the repo root.

For another Agent Skills client, copy `skills/openspider/` into that client's supported
skill directory and configure the MCP command `openspider mcp` separately. Client
support for discovery, namespacing and MCP differs; the portable skill has only the
standard `name` and `description` frontmatter and relative references. No client
configuration is written automatically.

From the repo root:

```sh
npm ci --prefix integrations
npm run --prefix integrations validate
npm test --prefix integrations
cargo build --locked -p os-cli -p os-testsite --bins
npm run --prefix integrations smoke
npm run --prefix integrations eval:report
```

Set `OPENSPIDER_CLAUDE_BINARY` to a Claude Code executable to include real marketplace
validation and installation in `smoke`. The CI matrix requires it on macOS and Windows.
The installation uses a disposable `CLAUDE_CONFIG_DIR`; the fixture crawl uses a
disposable `OPENSPIDER_HOME` and ephemeral loopback server. Smoke checks the installed
command artifact and executes its read-tool workflow, without an LLM or credentials.
Interactive slash invocation and behavioral skill grading remain optional/manual;
see [eval instructions](skills/openspider/evals/README.md). The CI summary explicitly
reports unmeasured agent success when no evaluation results were supplied.

After editing the canonical skill, run `npm run --prefix integrations sync`; validation
requires the plugin's shipped skill copy to be byte-identical. Real files avoid Windows
symlink requirements and preserve references when only the plugin directory is cached.
Evals are development assets and are not copied into the plugin.

Schemas in `schemas/` describe the subset used here and reject unexpected fields.
CI also runs the official `claude plugin validate` and marketplace installer. The tool
catalog check fails on an unknown Rust registration shape so a changed register cannot
silently leave stale documentation. No reference-product assets or text are used.

Node test dependencies: `ajv` (MIT), `yaml` (ISC), and their locked compatible transitive
dependencies. Claude Code is an external validation/installation tool, not a distributed
runtime dependency. The plugin launcher itself uses only Node standard libraries.
