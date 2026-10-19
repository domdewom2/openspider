//! Cookie Summary (OUT-111).
use crate::catalog::bulk_web::boolean;
use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::{ExportError, TableWriter};
use os_analyze::ColumnKind;
use os_model::Value;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Cookie Summary",
    aliases: &[],
    file_stem: "Cookie Summary",
    header: &[
        "Cookie Name",
        "Domain",
        "Path",
        "Persistence",
        "Secure",
        "HttpOnly",
        "SameSite",
        "Type",
        "First-Party",
        "URLs",
        "Sample URL",
    ],
    requires: &[ReportRequirement::Config {
        key: "extraction.cookies",
        what: "cookie extraction",
    }],
    owner: "M2-D18",
    run,
    header_fn: None,
}];

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Key(String, String, String, bool, bool, String, String);
struct Summary {
    cookie: crate::catalog::bulk_web::Cookie,
    urls: u64,
    last_url: i64,
}
fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    let mut groups = std::collections::BTreeMap::<Key, Summary>::new();
    // The source is ordered by URL: only the last URL per cookie key is needed to deduplicate instances.
    crate::catalog::bulk_web::visit_cookies(job, |cookie| {
        let key = Key(
            cookie.name.clone(),
            cookie.domain.clone(),
            cookie.path.clone(),
            cookie.secure,
            cookie.http_only,
            cookie.same_site.clone(),
            cookie.kind.clone(),
        );
        match groups.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Summary {
                    last_url: cookie.url_id,
                    cookie,
                    urls: 1,
                });
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let summary = entry.get_mut();
                if summary.last_url != cookie.url_id {
                    summary.urls += 1;
                    summary.last_url = cookie.url_id;
                }
                summary.cookie.first_party &= cookie.first_party;
            }
        }
        Ok(())
    })?;
    let count = groups.len() as u64;
    for summary in groups.into_values() {
        let c = summary.cookie;
        writer.row(
            &[
                ColumnKind::Text,
                ColumnKind::Text,
                ColumnKind::Text,
                ColumnKind::Text,
                ColumnKind::Bool,
                ColumnKind::Bool,
                ColumnKind::Text,
                ColumnKind::Text,
                ColumnKind::Bool,
                ColumnKind::Int,
                ColumnKind::Url,
            ],
            &[
                Value::Text(c.name.clone()),
                Value::Text(c.domain.clone()),
                Value::Text(c.path.clone()),
                c.persistence(),
                boolean(c.secure),
                boolean(c.http_only),
                Value::Text(c.same_site),
                Value::Text(c.kind),
                boolean(c.first_party),
                Value::Int(summary.urls as i64),
                Value::Text(c.address),
            ],
        )?;
    }
    Ok(count)
}
