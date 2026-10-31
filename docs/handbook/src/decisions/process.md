# Contribution and integration decisions

<a id="fixtures"></a>

## Fixture ownership

**Context.** Fixtures and shared tests are files that can conflict during integration.

**Decision.** Give each fixture an owner; the package-owned folder leads, with spec fixture names represented inside it.

**Consequences.** Read existing inputs as needed; change them only within the authorized package scope.

**Source:** M1 K-65; M2 K2-24. [M1 decision table](../../../plan/M1.md), [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [docs/plan/M2-packages.json](../../../../docs/plan/M2-packages.json).

<a id="layered-acceptance"></a>

## Acceptance across layers

**Context.** One requirement can involve core, commands, UI and integrations delivered separately.

**Decision.** Use spec_ids for the owning core package and extra_spec_ids for contributing layers; acceptance needs all applicable evidence.

**Consequences.** A green core test alone does not establish end-to-end completion.

**Source:** M2 K2-25. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [docs/plan/M2-packages.json](../../../../docs/plan/M2-packages.json), [crates/os-parity/src/coverage.rs](../../../../crates/os-parity/src/coverage.rs).

<a id="integration"></a>

## Minimal integration changes

**Context.** Replacing a stub or satisfying a spec can require changes outside a package boundary.

**Decision.** The general plan permits minimal, spec-required integration changes with a file/diff/Spec-ID record; expectation changes require their own justification.

**Consequences.** Explicit task restrictions still lead. Do not infer permission to edit unrelated paths or weaken test logic. The integrator resolves conflicts.

**Source:** M2 K2-60 / K2-63 / K2-71 / K2-72. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [docs/plan/M2.md](../../../../docs/plan/M2.md).

<a id="review"></a>

## Evidence before review

**Context.** Mechanical checks should be settled before substantive review.

**Decision.** Run the applicable ownership, spec-tag, lint, coverage and license checks first; scale review depth to risk and inspect CI evidence.

**Consequences.** Follow-up reviews can focus on reported findings; reviewers need a concrete diff and reproducible evidence.

**Source:** M2 K2-62. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [docs/plan/M2-check.py](../../../../docs/plan/M2-check.py).

<a id="ci"></a>

## Consolidated CI

**Context.** Repeated platform compilation and redundant runs delay feedback.

**Decision.** Use one consolidated main CI job per platform; macOS Intel joins nightly, manual and version-tag runs.

**Consequences.** Read actual workflow triggers and path filters before assuming a documentation-only commit ran every job.

**Source:** M2 K2-67. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [.github/workflows/ci.yml](../../../../.github/workflows/ci.yml).

<a id="tokens"></a>

## Semantic tokens and CSS checks

**Context.** Visual consistency needs shared tokens and measurable checks.

**Decision.** Use semantic tokens; the polish package calls for CSS grid/token lint. Current check:static runs source rules and token contrast tests.

**Consequences.** Do not describe the planned comprehensive CSS lint as already implemented. Check current static tests before adding styles.

**Source:** M2 K2-75a (U35); UI concept section 7.1. [M2 plan and decision table](../../../plan/M2.md), [UI concept](../../../plan/M2-ui-concept.md). **Implementation/evidence:** [apps/desktop/scripts/check-static.mjs](../../../../apps/desktop/scripts/check-static.mjs), [apps/desktop/src/ui/static/static.test.ts](../../../../apps/desktop/src/ui/static/static.test.ts).
