#![allow(dead_code)]
use os_model::*;
pub fn new_url(id: i64, internal: bool) -> NewUrl {
    let host = if internal { "site.test" } else { "other.test" };
    let url = format!("https://{host}/p{id}");
    NewUrl {
        id: UrlId(id),
        address: url.clone(),
        origin: format!("https://{host}"),
        url_length: u32::try_from(url.len()).unwrap(),
        url,
        is_internal: internal,
        in_tables: true,
        discovered_via: DiscoveryKind::Link,
        crawl_depth: Some(1),
        folder_depth: 0,
        state: UrlState::Queued,
        not_crawled_reason: None,
    }
}

pub fn link(target: Option<i64>, link_type: LinkType, position: u32) -> LinkRow {
    LinkRow {
        target_id: target.map(UrlId),
        target_raw: target.map_or("mailto:x@y.test".to_string(), |t| format!("/p{t}")),
        link_type,
        position,
        anchor: Some(format!("Anker {position}")),
        alt_text: None,
        follow: true,
        rel: None,
        target_attr: None,
        path_type: None,
        link_path: None,
        link_position: None,
        link_origin: LinkOrigin::Html,
        in_head: Some(false),
        ..Default::default()
    }
}

pub fn page(id: i64, links: Vec<LinkRow>) -> PageWrite {
    PageWrite {
        url_id: UrlId(id),
        replace: false,
        fetch: FetchFacts {
            crawl_timestamp: created_at(),
            status_code: 200,
            status: "OK".into(),
            content_type: Some("text/html".into()),
            content_class: ContentClass::Html,
            size_bytes: 1000,
            ..FetchFacts::default()
        },
        page: None,
        indexability: Indexability::Indexable,
        indexability_status: None,
        links,
        images: vec![],
        canonicals: vec![],
        directives: vec![],
        refresh: vec![],
        hreflang: vec![],
        pagination: vec![],
        headers: vec![],
        html: None,
    }
}

fn created_at() -> Timestamp {
    Timestamp::from_db("2026-10-18T12:00:00.000Z").unwrap()
}
