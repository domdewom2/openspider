//! Common representation: ordinary JSON-LD objects with reserved `_os_*` source metadata.
//! `_os_order` preserves property order independently of serde_json's map implementation.
use os_model::SdItem;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{json, Map, Value};
use std::fmt;

pub(crate) struct Ordered(pub Value);
impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON value")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Ordered, E> {
                Ok(Ordered(json!(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Ordered, E> {
                Ok(Ordered(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Ordered, E> {
                Ok(Ordered(json!(v)))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Ordered, E> {
                Ok(Ordered(json!(v)))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Ordered, E> {
                Ok(Ordered(json!(v)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Ordered, E> {
                Ok(Ordered(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Ordered, A::Error> {
                let mut values = Vec::new();
                while let Some(Ordered(v)) = a.next_element()? {
                    values.push(v);
                }
                Ok(Ordered(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Ordered, A::Error> {
                let mut values = Map::new();
                let mut order = Vec::new();
                while let Some((key, Ordered(v))) = a.next_entry::<String, Ordered>()? {
                    if key == "@type"
                        && !v.is_string()
                        && !v.as_array().is_some_and(|a| a.iter().all(Value::is_string))
                    {
                        return Err(serde::de::Error::custom(
                            "@type must be a string or an array of strings",
                        ));
                    }
                    if !key.starts_with("_os_") {
                        order.push(key.clone());
                        values.insert(key, v);
                    }
                }
                values.insert("_os_order".into(), json!(order));
                Ok(Ordered(Value::Object(values)))
            }
        }
        d.deserialize_any(V)
    }
}

pub fn short_type(iri: &str) -> &str {
    iri.strip_prefix("https://schema.org/")
        .or_else(|| iri.strip_prefix("http://schema.org/"))
        .or_else(|| iri.rsplit_once(['/', '#']).map(|(_, s)| s))
        .unwrap_or(iri)
}
pub fn types(v: &Value) -> Vec<&str> {
    match v.get("@type") {
        Some(Value::String(t)) => vec![t],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}
pub(crate) fn item(format: &str, mut value: Value, source: Value) -> SdItem {
    let type_path = types(&value)
        .iter()
        .map(|t| short_type(t))
        .collect::<Vec<_>>()
        .join("|");
    let short_types = types(&value)
        .iter()
        .map(|t| short_type(t).to_owned())
        .collect::<Vec<_>>();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_os_source".into(), source);
        obj.insert("_os_types".into(), json!(short_types));
    }
    SdItem {
        format: format.into(),
        type_path,
        json: value.to_string(),
        parse_error: None,
    }
}
pub(crate) fn error(
    format: &str,
    message: &str,
    block: usize,
    line: usize,
    column: usize,
    snippet: &str,
) -> SdItem {
    let error = json!({"message": message, "block": block, "line": line.max(1), "column": column.max(1), "snippet": snippet});
    SdItem {
        format: format.into(),
        type_path: String::new(),
        json: json!({"_os_parse_error":error}).to_string(),
        parse_error: Some(format!(
            "{message} (block {block}, line {}, column {})",
            line.max(1),
            column.max(1)
        )),
    }
}
pub(crate) fn add(obj: &mut Map<String, Value>, key: &str, value: Value) {
    let order = obj.entry("_os_order").or_insert_with(|| json!([]));
    if let Some(order) = order.as_array_mut() {
        if !order.iter().any(|v| v == key) {
            order.push(json!(key));
        }
    }
    match obj.get_mut(key) {
        None => {
            obj.insert(key.into(), value);
        }
        Some(Value::Array(a)) => a.push(value),
        Some(old) => {
            *old = json!([old.clone(), value]);
        }
    }
}
pub(crate) fn value(el: scraper::ElementRef<'_>, base: &url::Url, microdata: bool) -> String {
    let tag = el.value().name();
    let attr = if microdata {
        match tag {
            "meta" => Some("content"),
            "img" | "audio" | "video" | "source" | "iframe" | "embed" => Some("src"),
            "a" | "area" | "link" => Some("href"),
            "object" => Some("data"),
            "data" | "meter" => Some("value"),
            "time" if el.value().attr("datetime").is_some() => Some("datetime"),
            _ => None,
        }
    } else {
        ["content", "resource", "href", "src"]
            .into_iter()
            .find(|k| el.value().attr(k).is_some())
    };
    if let Some(attr) = attr {
        let raw = el.value().attr(attr).unwrap_or("");
        if matches!(attr, "href" | "src" | "data" | "resource") {
            return base
                .join(raw)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| raw.into());
        }
        return raw.into();
    }
    el.text().collect::<String>()
}
