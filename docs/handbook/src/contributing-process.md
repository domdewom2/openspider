# Contributing process

A requirement describes behavior; a work package defines a bounded change; a plan assigns ownership, dependencies and acceptance. Start with [CONTRIBUTING.md](../../../CONTRIBUTING.md), [repository rules](../../../CLAUDE.md), the [spec index](../../spec/README.md) and the applicable [M1](../../plan/M1.md) / [M2](../../plan/M2.md) package entries.

## From requirement to reviewed change

1. Identify the relevant Spec-IDs and acceptance criteria. Read existing code and fixtures. Ask for a specification clarification when the required behavior is unclear; do not derive it by inspecting the reference product. [CLAUDE.md](../../../CLAUDE.md) defines the clean-room boundary.
2. Confirm package dependencies have been integrated and work in the assigned branch/worktree. Respect `owns`, including tests, fixtures and generated files. [M2-packages.json](../../plan/M2-packages.json) records assignments; [M2-check.py](../../plan/M2-check.py) checks plan structure and ownership constraints.
3. Implement the assigned layer. `spec_ids` and `extra_spec_ids` distinguish requirement ownership from additional layer evidence (K2-25). Add fixture-backed tests with [Spec-ID tags](testing.md), and preserve public contracts and platform neutrality.
4. Run applicable [handoff checks](getting-started.md), review the diff and commit only explicit owned paths. Keep commits small with a scope prefix, for example `fetch:`, `parse:`, `ui:` or `docs:`. These are [repository rules](../../../CLAUDE.md).
5. Provide a reviewable diff and evidence: covered IDs/criteria, checks and results, unresolved questions, required integration changes and relevant CI run. The [M2 process](../../plan/M2.md) defines review and acceptance. State any package-specific check restrictions rather than claiming unexecuted checks passed.

## Ownership and integration

Do not stage unrelated changes or reset another contributor's work. Use `git add <explicit paths>`, never `git add .`, `git add -A` or `git commit -a` ([CLAUDE.md](../../../CLAUDE.md)). A package's explicit restrictions take precedence over general plan exceptions.

The general plan permits specific minimal integration fixes and stub replacement, with per-file rationale and Spec-IDs (K2-60/63/71/72). The [decision entry](decisions/process.md#integration) explains the boundary. Central manifests and lockfiles have designated owners; coordinate dependency changes instead of editing them incidentally. See [M2 C7/C10](../../plan/M2.md).

The integrator merges accepted package branches in dependency order, resolves integration conflicts and checks the combined result. The plan uses non-fast-forward merge commits and separate follow-up packages for cross-cutting findings. Gates evaluate integrated evidence; they do not turn a pending criterion into a passing one. See [M2 C1/C6/C9](../../plan/M2.md) and [M1 integration lessons](../../plan/M1.md).

## Public contribution records

Keep descriptions focused on behavior, sources and verification. Do not include credentials, personal data, internal hosts or machine-specific paths. Preserve the [neutrality policy](../../../scripts/check-neutral.sh). Test with local fixtures and memory secrets as required by [M2 C2/C13](../../plan/M2.md). Agent contributions follow the same ownership, clean-room and evidence rules as human contributions.
