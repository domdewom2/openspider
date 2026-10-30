# Crawl file

A crawl is a SQLite `.ospider` file. The [store](../../../../crates/os-store/src/lib.rs) owns opening, saving, migration and queries. [Storage profiles](../../../../crates/os-store/src/profile.rs) use WAL mode; active writes can therefore have SQLite sidecar files. Use the [file/open APIs](../../../../crates/os-store/src/open.rs) for supported save and copy behavior rather than copying an active main database alone.

## Identity and migration

The [schema registry](../../../../crates/os-store/src/schema.rs) currently defines `application_id = 0x4F535044` (OSPD), schema version **5**, and public read API version **1**. `PRAGMA user_version` records the schema version. Migrations run in order; add a new migration and registry entry instead of editing a migration already shipped.

| Migration | Purpose |
|---|---|
| [0001](../../../../crates/os-store/migrations/0001_init.sql) | Initial URLs, links, metadata, queue and related records |
| [0002](../../../../crates/os-store/migrations/0002_url_counters.sql) | Separate `url_counters` as the maintained source of link counters |
| [0003](../../../../crates/os-store/migrations/0003_m2.sql) | M2 records and embedded public `q_*` read views |
| [0004](../../../../crates/os-store/migrations/0004_w1.sql) | Further M2 records including discovery and analysis data |
| [0005](../../../../crates/os-store/migrations/0005_compact_ext.sql) | Compact extension-key storage while preserving the `url_ext` SQL view |

Internal tables are implementation details. In particular, do not infer current link counters from legacy columns on `urls`: read [counter code](../../../../crates/os-store/src/counters.rs) and [migration 0002](../../../../crates/os-store/migrations/0002_url_counters.sql).

## Public read interface

External scripts should use `q_manifest`, `q_config`, `q_urls`, `q_links`, `q_sitemap_urls` and `q_columns`, documented in [read_api.rs](../../../../crates/os-store/src/read_api.rs). These views live in the file and need standard SQLite, without OpenSpider collations or functions. Check `q_manifest` for format, completeness and read API version; use `q_columns` for self-description before selecting fields. Open external consumers read-only. See [OUT-233/234](../../../spec/06-output-ops.md) for the contract and [Linux read-API smoke](../../../../.github/workflows/linux-core.yml) for an independent consumer.

## Durability and derived data

[SqliteSink](../../../../crates/os-store/src/writer.rs) writes crawl batches; the [background checkpointer](../../../../crates/os-store/src/checkpointer.rs) handles WAL work. [Profiles](../../../../crates/os-store/src/profile.rs) trade cache and synchronization settings: Balanced uses 256 MiB cache, MinMemory 64 MiB, MaxSpeed a RAM-dependent cache with `synchronous=OFF` during crawling and `NORMAL` afterward. MaxSpeed changes durability during a run; it is not an alternate file format.

The frontier's [disk queue](../../../../crates/os-engine/src/queue_disk.rs) and [resume path](../../../../crates/os-store/src/resume.rs) support continuation. [View caches](../../../../crates/os-store/src/view_cache.rs) are derived data. Their presence must not become a prerequisite for interpreting primary crawl facts; see [view-session decision](../decisions/architecture.md#view-sessions).
