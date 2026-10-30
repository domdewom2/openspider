# Architecture and storage decisions

<a id="page-trees"></a>

## Bounded page trees

**Context.** HTML head semantics require an HTML5 tree builder.

**Decision.** Parse a tree per page, bounded by the configured page-size limit, then release it.

**Consequences.** Do not retain a crawl-wide DOM.

**Source:** M1 K-09. [M1 decision table](../../../plan/M1.md). **Implementation/evidence:** [crates/os-parse/src/dom.rs](../../../../crates/os-parse/src/dom.rs).

<a id="storage-profiles"></a>

## Storage profiles

**Context.** Cache size and synchronization affect throughput and memory.

**Decision.** Use Balanced, MinMemory or MaxSpeed storage profiles; MaxSpeed caps its RAM-dependent cache.

**Consequences.** Keep profile choices separate from data semantics; inspect durability settings before performance comparisons.

**Source:** M1 K-16. [M1 decision table](../../../plan/M1.md). **Implementation/evidence:** [crates/os-store/src/profile.rs](../../../../crates/os-store/src/profile.rs).

<a id="link-counters"></a>

## Live link counters

**Context.** Wide random URL updates and maintaining the link-target index during crawling are expensive.

**Decision.** Maintain link counters in url_counters; defer the target index during the write run and build it at completion.

**Consequences.** Use maintained counters for live totals, and avoid a full links scan per result row.

**Source:** M1 K-74 / K-75. [M1 decision table](../../../plan/M1.md). **Implementation/evidence:** [crates/os-store/src/counters.rs](../../../../crates/os-store/src/counters.rs), [crates/os-store/src/link_index.rs](../../../../crates/os-store/src/link_index.rs).

<a id="checkpoint"></a>

## Background WAL work

**Context.** Checkpoint work on the writer path can stall commits.

**Decision.** Use a separate connection for passive background checkpoints and disable writer auto-checkpointing.

**Consequences.** Keep final lifecycle checkpoints distinct from background work.

**Source:** M1 K-76. [M1 decision table](../../../plan/M1.md). **Implementation/evidence:** [crates/os-store/src/checkpointer.rs](../../../../crates/os-store/src/checkpointer.rs), [crates/os-store/src/writer.rs](../../../../crates/os-store/src/writer.rs).

<a id="bounded-frontier"></a>

## Bounded frontier memory

**Context.** Keeping the entire queue and full seen URLs in memory scales poorly.

**Decision.** Keep a bounded queue window backed by the crawl file and a compact seen structure.

**Consequences.** Preserve priority/depth/sequence order and deterministic resume when changing admission or refill logic.

**Source:** M1 K-78. [M1 decision table](../../../plan/M1.md). **Implementation/evidence:** [crates/os-engine/src/queue_disk.rs](../../../../crates/os-engine/src/queue_disk.rs), [crates/os-engine/src/seen.rs](../../../../crates/os-engine/src/seen.rs).

<a id="view-sessions"></a>

## View sessions

**Context.** Large tables need random-access windows without loading every row into the UI.

**Decision.** Treat filter bitmaps as derived caches; distinguish a view generation from the database change sequence.

**Consequences.** A filter/sort change can invalidate a response even when stored crawl facts did not change.

**Source:** UI concept K-1; M2 K2-37. [M2 plan and decision table](../../../plan/M2.md), [UI concept](../../../plan/M2-ui-concept.md). **Implementation/evidence:** [crates/os-analyze/src/views/session/mod.rs](../../../../crates/os-analyze/src/views/session/mod.rs), [apps/desktop/src/ipc/client.ts](../../../../apps/desktop/src/ipc/client.ts).

<a id="read-connections"></a>

## Read connections and serialized writes

**Context.** Database calls must not block the UI or Tokio workers, and app writes must be coordinated.

**Decision.** Run database reads through the blocking read pool and app writes through one queue.

**Consequences.** Current pool code defaults to four read connections per crawl and exposes cached read slots for workload isolation. Inspect those slots rather than assuming an undocumented lane topology.

**Source:** UI concept K-20; M2 section 3.6. [M2 plan and decision table](../../../plan/M2.md), [UI concept](../../../plan/M2-ui-concept.md). **Implementation/evidence:** [crates/os-app/src/pool.rs](../../../../crates/os-app/src/pool.rs), [crates/os-app/src/write_queue.rs](../../../../crates/os-app/src/write_queue.rs).

<a id="resource-guard"></a>

## Resource guard

**Context.** Memory protection must cover the engine, renderer and downstream operations.

**Decision.** Production RAM settings retain a minimum of 1 GiB; smaller limits are injected only for tests. Apply cooperative checkpoints to analysis and export.

**Consequences.** Do not turn a small acceptance-test injection into a user setting; preserve pause/resume behavior.

**Source:** M2 K2-56. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [crates/os-engine/src/memory.rs](../../../../crates/os-engine/src/memory.rs), [crates/os-model/src/cancel.rs](../../../../crates/os-model/src/cancel.rs).

<a id="shared-scheduling"></a>

## Shared scheduling

**Context.** App and CLI both need task scheduling without depending on one another.

**Decision.** Put task definitions, history, planner adapters and mail in os-sched.

**Consequences.** Keep scheduling reusable below the desktop service layer.

**Source:** M2 K2-07. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [crates/os-sched/src/lib.rs](../../../../crates/os-sched/src/lib.rs).

<a id="chromium"></a>

## External Chromium

**Context.** Rendering needs a browser installation strategy.

**Decision.** For M2 use system Chrome/Chromium or a configured browser path; bundling is a later release decision.

**Consequences.** Report a missing browser clearly; CI and local tests must select an executable explicitly when needed.

**Source:** M2 K2-10. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [crates/os-render/src/lib.rs](../../../../crates/os-render/src/lib.rs), [scripts/ci/linux-core.sh](../../../../scripts/ci/linux-core.sh).

<a id="network"></a>

## Shared network settings

**Context.** Crawl, API, renderer and mail network settings must agree.

**Decision.** Use app-wide NetworkSettings and the shared fetch network builder as the HTTP entry point.

**Consequences.** Avoid adding an unrelated proxy or certificate configuration in each adapter.

**Source:** M2 K2-33. [M2 plan and decision table](../../../plan/M2.md). **Implementation/evidence:** [crates/os-fetch/src/net.rs](../../../../crates/os-fetch/src/net.rs), [crates/os-model/src/settings.rs](../../../../crates/os-model/src/settings.rs).

<a id="text-collation"></a>

## Shared text collation

**Context.** Text ordering and duplicate comparison must agree across readers.

**Decision.** Use OS_TEXT (ICU root, secondary strength) for text comparisons; URL ordering remains bytewise.

**Consequences.** Initialize collation on internal SQLite connections and use the same comparator in Rust-side view sorting.

**Source:** M1 K-70; UI concept K-18. [M1 decision table](../../../plan/M1.md), [UI concept](../../../plan/M2-ui-concept.md). **Implementation/evidence:** [crates/os-store/src/collation.rs](../../../../crates/os-store/src/collation.rs).
