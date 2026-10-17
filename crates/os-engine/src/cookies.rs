//! Verlustfreie HTTP-Cookie-Erfassung, unabhängig vom Sitzungsspeicher (CRAWL-062).
use os_model::{CookieOrigin, CookieWrite, UrlId};

pub fn collect(url_id: UrlId, raw: &[String]) -> Vec<CookieWrite> {
    raw.iter()
        .enumerate()
        .filter_map(|(position, raw)| {
            let mut attrs = raw.split(';');
            let (name, value) = attrs.next()?.split_once('=')?;
            let mut cookie = CookieWrite {
                url_id,
                origin: CookieOrigin::Http,
                position: position as u32,
                name: name.trim().to_string(),
                value: value.trim().to_string(),
                domain: None,
                path: None,
                expires: None,
                max_age: None,
                secure: false,
                http_only: false,
                same_site: None,
                raw: raw.clone(),
            };
            if cookie.name.is_empty() {
                return None;
            }
            for attr in attrs {
                let (name, value) = attr.trim().split_once('=').unwrap_or((attr.trim(), ""));
                let value = value.trim();
                match name.trim().to_ascii_lowercase().as_str() {
                    "domain" => cookie.domain = Some(value.to_string()),
                    "path" => cookie.path = Some(value.to_string()),
                    "expires" => cookie.expires = Some(value.to_string()),
                    "max-age" => cookie.max_age = value.parse().ok(),
                    "secure" => cookie.secure = true,
                    "httponly" => cookie.http_only = true,
                    "samesite" => cookie.same_site = Some(value.to_string()),
                    _ => {}
                }
            }
            Some(cookie)
        })
        .collect()
}
