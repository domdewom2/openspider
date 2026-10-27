use super::super::{CompareError, CrawlSide};
use os_model::{ChangeDetectionConfig, ChangeElement};
use rusqlite::{types::Value, OptionalExtension, Statement};
use std::collections::BTreeSet;

pub(super) struct Page {
    pub address: String,
    values: Vec<Option<Value>>,
}
impl Page {
    pub fn value(&self, e: ChangeElement) -> Option<&Value> {
        let i = super::ELEMENTS
            .iter()
            .position(|(known, _, _)| *known == e)?;
        self.values[i].as_ref()
    }
}

pub(super) struct Reader<'a> {
    side: &'a CrawlSide,
    row: Statement<'a>,
    ext: Statement<'a>,
    sd: Statement<'a>,
    sig: Statement<'a>,
    pub reasons: String,
    pub types: String,
}
impl<'a> Reader<'a> {
    pub fn new(side: &'a CrawlSide) -> Result<Self, CompareError> {
        let conn = side.file.conn();
        Ok(Self {side,row:conn.prepare("SELECT u.address,u.indexability,u.title_1,u.meta_desc_1,u.h1_1,u.word_count,u.crawl_depth,c.inlinks,c.unique_inlinks,c.outlinks,c.unique_outlinks,c.external_outlinks,c.unique_external_outlinks,u.indexability_status FROM urls u LEFT JOIN url_counters c ON c.url_id=u.id WHERE u.id=?1 AND u.is_internal=1 AND u.in_tables=1 AND u.state='done' AND u.content_class='HTML' AND u.status_code>0")?,ext:conn.prepare("SELECT v_text FROM url_ext WHERE url_id=?1 AND key='content.text'")?,sd:conn.prepare("SELECT json FROM sd_items WHERE url_id=?1 AND ((format='json-ld' AND ?2) OR (format='microdata' AND ?3) OR (format='rdfa' AND ?4)) ORDER BY id")?,sig:conn.prepare("SELECT sig FROM minhash WHERE url_id=?1")?,reasons:String::new(),types:String::new()})
    }
    pub fn read(
        &mut self,
        id: i64,
        cfg: &ChangeDetectionConfig,
    ) -> Result<Option<Page>, CompareError> {
        let result = self
            .row
            .query_row([id], |r| {
                let mut values = Vec::new();
                for i in 1..13 {
                    values.push(Some(r.get::<_, Value>(i)?));
                }
                Ok((
                    r.get::<_, String>(0)?,
                    values,
                    r.get::<_, Option<String>>(13)?.unwrap_or_default(),
                ))
            })
            .optional()?;
        let Some((address, mut values, reasons)) = result else {
            return Ok(None);
        };
        self.reasons = set(&reasons);
        for index in [0, 4, 5, 6, 7, 8, 9, 10, 11] {
            if matches!(values[index], Some(Value::Null)) {
                values[index] = None;
            }
        }
        let config = &self.side.config.extraction;
        for (index, enabled) in [
            (0, config.indexability),
            (1, config.page_titles),
            (2, config.meta_description),
            (3, config.h1),
            (4, config.word_count),
        ] {
            if !enabled {
                values[index] = None;
            }
        }
        // Normalisierter, case-sensitiver erster Text. Fehlend entspricht leer; deaktiviert bleibt None.
        for v in values[1..4].iter_mut().flatten() {
            *v = Value::Text(match v {
                Value::Text(s) => s.split_whitespace().collect::<Vec<_>>().join(" "),
                _ => String::new(),
            });
        }
        let sd = &config.structured_data;
        self.types.clear();
        if sd.json_ld || sd.microdata || sd.rdfa {
            let mut names = BTreeSet::new();
            for json in self.sd.query_map(
                rusqlite::params![id, sd.json_ld, sd.microdata, sd.rdfa],
                |r| r.get::<_, String>(0),
            )? {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json?) {
                    collect_types(&value, &mut names);
                }
            }
            values.push(Some(Value::Integer(names.len() as i64)));
            self.types = names.into_iter().collect::<Vec<_>>().join("; ");
        } else {
            values.push(None);
        }
        let signature = if cfg.elements.contains(&ChangeElement::Content) {
            self.sig
                .query_row([id], |r| r.get::<_, Vec<u8>>(0))
                .optional()?
        } else {
            None
        };
        let signature = if signature.is_none() && cfg.elements.contains(&ChangeElement::Content) {
            self.ext
                .query_row([id], |r| r.get::<_, Option<String>>(0))
                .optional()?
                .flatten()
                .map(|text| {
                    crate::minhash::signature(&crate::minhash::shingles(&text))
                        .iter()
                        .flat_map(|v| v.to_le_bytes())
                        .collect()
                })
        } else {
            signature
        };
        values.push(signature.map(Value::Blob));
        Ok(Some(Page { address, values }))
    }
}
fn set(text: &str) -> String {
    text.split([';', ',', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("; ")
}
fn collect_types(value: &serde_json::Value, names: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(o) => {
            if let Some(t) = o.get("_os_types").or_else(|| o.get("@type")) {
                for value in t
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or(std::slice::from_ref(t))
                {
                    if let Some(s) = value.as_str() {
                        names.insert(
                            s.trim_start_matches("https://schema.org/")
                                .trim_start_matches("http://schema.org/")
                                .to_string(),
                        );
                    }
                }
            }
            for v in o.values() {
                collect_types(v, names);
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                collect_types(v, names);
            }
        }
        _ => {}
    }
}
