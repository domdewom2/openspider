//! Flüchtiger HSTS-Speicher je Fetcher. Keine Preload-Liste, keine Persistenz.
use std::{collections::HashMap, sync::Mutex, time::Duration};
use url::Url;
#[derive(Default)]
pub(super) struct Policies(Mutex<HashMap<String, Policy>>);
struct Policy {
    expires: Duration,
    subdomains: bool,
}
impl Policies {
    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).is_empty()
    }
    pub fn learn(&self, url: &Url, value: &str, now: Duration) {
        if url.scheme() != "https"
            || url.host_str().is_none()
            || url.host_str().unwrap().parse::<std::net::IpAddr>().is_ok()
        {
            return;
        }
        let mut age = None;
        let mut subdomains = false;
        for part in value.split(';').map(str::trim) {
            if let Some((key, val)) = part.split_once('=') {
                if key.eq_ignore_ascii_case("max-age") {
                    if age.is_some() {
                        return;
                    }
                    let Ok(n) = val.trim().trim_matches('"').parse::<u64>() else {
                        return;
                    };
                    age = Some(n);
                }
            } else if part.eq_ignore_ascii_case("includesubdomains") {
                subdomains = true;
            }
        }
        let Some(age) = age else {
            return;
        };
        let host = url
            .host_str()
            .unwrap()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let mut policies = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if age == 0 {
            policies.remove(&host);
        } else {
            policies.insert(
                host,
                Policy {
                    expires: now.saturating_add(Duration::from_secs(age)),
                    subdomains,
                },
            );
        }
    }
    pub fn upgrade(&self, url: &Url, now: Duration) -> Option<Url> {
        if url.scheme() != "http" {
            return None;
        }
        let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
        let mut policies = self.0.lock().unwrap_or_else(|p| p.into_inner());
        policies.retain(|_, p| p.expires > now);
        if !policies
            .iter()
            .any(|(h, p)| *h == host || (p.subdomains && host.ends_with(&format!(".{h}"))))
        {
            return None;
        }
        let port = url.port();
        let mut target = url.clone();
        target.set_scheme("https").ok()?;
        target.set_port(port).ok()?;
        Some(target)
    }
}
