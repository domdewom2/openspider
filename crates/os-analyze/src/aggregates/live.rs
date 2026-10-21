//! Inkrementelle Aggregate (UI-066, UI-069, UI-145).
//!
//! Je URL wird nur ihr alter Beitrag auf Platte vorgehalten. Ein Update zieht ihn
//! ab und addiert den neuen: Wiederholungen zählen nie doppelt. Temp-Tabellen sind
//! verbindungslokal und funktionieren auch bei einer schreibgeschützten Hauptdatei.
use os_model::{CrawlConfig, UrlId};
use os_store::{CrawlFile, StoreError};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

pub(crate) const SELECT: &str = "SELECT id, url, is_internal, content_class, indexability, crawl_depth,
    status_code, robots_blocked, response_time_ms, state, no_response_reason, in_tables FROM main.urls";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Contribution {
    pub(crate) paths: Vec<(String, String, String)>, // path, parent, name
    pub(crate) depth: Option<u32>,
    pub(crate) indexable: bool,
    pub(crate) non_indexable: bool,
    pub(crate) response_class: usize,
    time: Option<i32>,
    internal_html: bool,
}

pub(crate) fn contribution(r: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, Contribution)> {
    let url: String = r.get(1)?;
    let internal: bool = r.get(2)?;
    let visible: bool = r.get(11)?;
    let html = r.get::<_, String>(3)? == "HTML";
    let indexable = r.get::<_, Option<String>>(4)?.as_deref() == Some("Indexable");
    let depth = r
        .get::<_, Option<u32>>(5)?
        .filter(|d| *d > 0)
        .map(|d| d.min(10));
    let status: Option<u16> = r.get(6)?;
    let blocked: bool = r.get(7)?;
    let ms: Option<u32> = r.get(8)?;
    let state: String = r.get(9)?;
    let failed = r.get::<_, Option<String>>(10)?.is_some();
    let response_class = if blocked {
        4
    } else {
        match status.map(|s| s / 100) {
            Some(2) => 0,
            Some(3) => 1,
            Some(4) => 2,
            Some(5) => 3,
            _ => 5,
        }
    };
    // Nur abgeschlossene Abrufe, einschließlich Fehler. Weder noch nicht besuchte
    // URLs noch durch robots/Regeln übersprungene URLs sind Antwortzeit-Messungen.
    let attempted = status.is_some() || failed || state == "done";
    let time = if attempted && !blocked && visible {
        if status.is_none() {
            Some(-1)
        } else {
            ms.map(|ms| (ms / 1000).min(10) as i32)
        }
    } else {
        None
    };
    Ok((
        r.get(0)?,
        Contribution {
            paths: if internal && visible {
                directory_paths(&url)
            } else {
                Vec::new()
            },
            depth: if internal && visible { depth } else { None },
            indexable: html && indexable,
            non_indexable: html && !indexable,
            response_class,
            time,
            internal_html: internal && html && visible,
        },
    ))
}

fn directory_paths(url: &str) -> Vec<(String, String, String)> {
    let url = url.split(['?', '#']).next().unwrap_or(url);
    let Some((scheme, rest)) = url.split_once("://") else {
        return Vec::new();
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let root = format!("{scheme}://{host}/");
    let mut out = vec![(root.clone(), String::new(), host.to_owned())];
    let mut parent = root;
    // Der letzte Bestandteil ist die Seite (auch ohne Dateiendung), außer bei
    // abschließendem Slash: dann gehört das Verzeichnis selbst zum Pfad.
    let mut parts: Vec<_> = path.split('/').collect();
    parts.pop();
    for part in parts {
        let next = format!("{parent}{part}/");
        out.push((next.clone(), parent, part.to_owned()));
        parent = next;
    }
    out
}

fn init(file: &CrawlFile) -> Result<(), StoreError> {
    file.conn().execute_batch(
        "PRAGMA temp.cache_size=-4096;
         CREATE TEMP TABLE IF NOT EXISTS k03_rows(id INTEGER PRIMARY KEY, contribution TEXT NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS k03_meta(id INTEGER PRIMARY KEY CHECK(id=1), seq INTEGER NOT NULL, reset TEXT NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS k03_counts(
           kind TEXT NOT NULL, key TEXT NOT NULL, parent TEXT NOT NULL, name TEXT NOT NULL,
           n INTEGER NOT NULL, idx INTEGER NOT NULL, nonidx INTEGER NOT NULL,
           s2 INTEGER NOT NULL, s3 INTEGER NOT NULL, s4 INTEGER NOT NULL, s5 INTEGER NOT NULL,
           blocked INTEGER NOT NULL, noresp INTEGER NOT NULL, PRIMARY KEY(kind,key)) WITHOUT ROWID;
         CREATE INDEX IF NOT EXISTS temp.k03_parent ON k03_counts(kind,parent,key);",
    )?;
    Ok(())
}

fn json_error(e: serde_json::Error) -> StoreError {
    StoreError::InvalidInput(format!("Aggregate: {e}"))
}

fn add(file: &CrawlFile, c: &Contribution, sign: i64) -> Result<(), StoreError> {
    let mut stmt = file.conn().prepare_cached(
        "INSERT INTO temp.k03_counts(kind,key,parent,name,n,idx,nonidx,s2,s3,s4,s5,blocked,noresp)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(kind,key) DO UPDATE SET n=n+excluded.n, idx=idx+excluded.idx, nonidx=nonidx+excluded.nonidx,
           s2=s2+excluded.s2,s3=s3+excluded.s3,s4=s4+excluded.s4,s5=s5+excluded.s5,
           blocked=blocked+excluded.blocked,noresp=noresp+excluded.noresp",
    )?;
    let mut put = |kind: &str, key: &str, parent: &str, name: &str| -> Result<(), StoreError> {
        let classes: [i64; 6] =
            std::array::from_fn(|i| if c.response_class == i { sign } else { 0 });
        stmt.execute(params![
            kind,
            key,
            parent,
            name,
            sign,
            sign * i64::from(c.indexable),
            sign * i64::from(c.non_indexable),
            classes[0],
            classes[1],
            classes[2],
            classes[3],
            classes[4],
            classes[5]
        ])?;
        Ok(())
    };
    for (path, parent, name) in &c.paths {
        put("path", path, parent, name)?;
    }
    if let Some(depth) = c.depth {
        put("depth", &depth.to_string(), "", "")?;
    }
    if let Some(time) = c.time {
        put("time_all", &time.to_string(), "", "")?;
        if c.internal_html {
            put("time_html", &time.to_string(), "", "")?;
        }
    }
    Ok(())
}

fn remove(file: &CrawlFile, id: i64) -> Result<(), StoreError> {
    let old: Option<String> = file
        .conn()
        .query_row(
            "SELECT contribution FROM temp.k03_rows WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(old) = old {
        add(file, &serde_json::from_str(&old).map_err(json_error)?, -1)?;
        file.conn()
            .execute("DELETE FROM temp.k03_rows WHERE id=?1", [id])?;
    }
    Ok(())
}

fn replace(file: &CrawlFile, id: i64, c: Contribution) -> Result<(), StoreError> {
    remove(file, id)?;
    add(file, &c, 1)?;
    file.conn().execute(
        "INSERT INTO temp.k03_rows(id,contribution) VALUES(?1,?2)",
        params![id, serde_json::to_string(&c).map_err(json_error)?],
    )?;
    Ok(())
}

fn check_cancel(cancel: &os_model::CancelToken) -> Result<(), StoreError> {
    if cancel.is_cancelled() {
        return Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_INTERRUPT),
            None,
        )));
    }
    Ok(())
}

/// Bringt den Cache innerhalb eines konsistenten Snapshots auf den Feed-Stand.
/// Der Erstaufbau streamt Zeilen; laufende Aktualisierungen lesen nur den Index
/// `urls_change_seq`. Löschungen werden bei geändertem `reset_seq` abgeglichen.
pub fn refresh(file: &CrawlFile, cfg: &CrawlConfig) -> Result<(), StoreError> {
    refresh_with_cancel(file, cfg, &os_model::CancelToken::new())
}

/// Abbrechbarer Erstaufbau und Feed-Nachlauf; auch die Rust-Schleife prüft die
/// Marke, damit sehr viele kurze SQLite-Anweisungen den Interrupt nicht umgehen.
pub fn refresh_with_cancel(
    file: &CrawlFile,
    _cfg: &CrawlConfig,
    cancel: &os_model::CancelToken,
) -> Result<(), StoreError> {
    check_cancel(cancel)?;
    init(file)?;
    let tx = file.conn().unchecked_transaction()?;
    let seq = os_store::queries::change_seq(file.conn())?;
    let reset = file.get_meta("reset_seq")?.unwrap_or_default();
    let previous: Option<(u64, String)> = file
        .conn()
        .query_row("SELECT seq,reset FROM temp.k03_meta WHERE id=1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    if previous
        .as_ref()
        .is_some_and(|(old, old_reset)| *old == seq && *old_reset == reset)
    {
        return Ok(());
    }
    if previous.as_ref().is_some_and(|(old, _)| *old > seq) {
        file.conn()
            .execute_batch("DELETE FROM temp.k03_rows; DELETE FROM temp.k03_counts;")?;
    }
    let since = previous
        .as_ref()
        .filter(|(old, _)| *old <= seq)
        .map(|(old, _)| *old);
    if previous.as_ref().is_some_and(|(_, old)| *old != reset) {
        let mut after = i64::MIN;
        loop {
            // Kein Löschen aus einer Tabelle mit noch offenem Zeilencursor.
            let mut stmt = file.conn().prepare("SELECT id FROM temp.k03_rows c WHERE id>?1 AND NOT EXISTS(SELECT 1 FROM main.urls u WHERE u.id=c.id) ORDER BY id LIMIT 4096")?;
            let ids = stmt
                .query_map([after], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            if ids.is_empty() {
                break;
            }
            after = *ids.last().expect("nicht leer");
            for id in ids {
                check_cancel(cancel)?;
                remove(file, id)?;
            }
        }
    }
    let sql = if since.is_some() {
        format!("{SELECT} WHERE change_seq > ?1")
    } else {
        SELECT.to_owned()
    };
    let mut stmt = file.conn().prepare(&sql)?;
    if let Some(since) = since {
        stmt.raw_bind_parameter(1, since)?;
    }
    let mut rows = stmt.raw_query();
    while let Some(row) = rows.next()? {
        check_cancel(cancel)?;
        let (id, c) = contribution(row)?;
        replace(file, id, c)?;
    }
    file.conn().execute(
        "INSERT OR REPLACE INTO temp.k03_meta(id,seq,reset) VALUES(1,?1,?2)",
        params![seq, reset],
    )?;
    tx.commit()?;
    Ok(())
}

/// F05-Vertrag: führt genau die genannten Änderungen einschließlich Löschungen
/// aus. Wiederholungen sind idempotent. Der Feed-Wasserstand bleibt unverändert,
/// damit nicht genannte Änderungen beim nächsten `refresh` weiter eintreffen.
pub fn apply_changes(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    ids: &[UrlId],
) -> Result<(), crate::AnalyzeError> {
    init(file)?;
    let initialized: bool =
        file.conn()
            .query_row("SELECT EXISTS(SELECT 1 FROM temp.k03_meta)", [], |r| {
                r.get(0)
            })?;
    if !initialized {
        refresh(file, cfg)?;
        return Ok(());
    }
    let tx = file.conn().unchecked_transaction()?;
    let mut stmt = file
        .conn()
        .prepare_cached(&format!("{SELECT} WHERE id=?1"))?;
    for id in ids {
        match stmt.query_row([id.0], contribution).optional()? {
            Some((id, c)) => replace(file, id, c)?,
            None => remove(file, id.0)?,
        }
    }
    tx.commit().map_err(StoreError::from)?;
    Ok(())
}
