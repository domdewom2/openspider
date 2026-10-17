//! RFC 7616, qop=auth, MD5/SHA-256 und ihre sess-Varianten.
use md5::{Digest, Md5};
use sha2::Sha256;
use std::{collections::HashMap, hash::BuildHasher, sync::Mutex};
use url::Url;
#[derive(Clone)]
struct Challenge {
    realm: String,
    nonce: String,
    opaque: Option<String>,
    algorithm: String,
    nc: u32,
}
#[derive(Default)]
pub(crate) struct DigestAuth(Mutex<HashMap<String, Challenge>>);
impl DigestAuth {
    pub fn challenge(&self, key: &str, value: &str) -> bool {
        let Some((scheme, rest)) = value.split_once(' ') else {
            return false;
        };
        if !scheme.eq_ignore_ascii_case("digest") {
            return false;
        }
        let Some(fields) = fields(rest) else {
            return false;
        };
        let Some(realm) = fields.get("realm") else {
            return false;
        };
        let Some(nonce) = fields.get("nonce").filter(|v| !v.is_empty()) else {
            return false;
        };
        if !fields
            .get("qop")
            .is_some_and(|q| q.split(',').any(|v| v.trim().eq_ignore_ascii_case("auth")))
        {
            return false;
        }
        let algorithm = fields
            .get("algorithm")
            .map(|a| a.to_ascii_uppercase())
            .unwrap_or_else(|| "MD5".into());
        if !matches!(
            algorithm.as_str(),
            "MD5" | "MD5-SESS" | "SHA-256" | "SHA-256-SESS"
        ) {
            return false;
        }
        let mut cache = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let nc = cache
            .get(key)
            .filter(|c| c.nonce == *nonce)
            .map_or(0, |c| c.nc);
        cache.insert(
            key.into(),
            Challenge {
                realm: realm.clone(),
                nonce: nonce.clone(),
                opaque: fields.get("opaque").cloned(),
                algorithm,
                nc,
            },
        );
        true
    }
    pub fn authorization(
        &self,
        key: &str,
        url: &Url,
        username: &str,
        password: &str,
    ) -> Option<String> {
        let mut cache = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let c = cache.get_mut(key)?;
        c.nc = c.nc.checked_add(1)?;
        let cnonce = format!(
            "{:016x}",
            std::collections::hash_map::RandomState::new().hash_one((&c.nonce, c.nc))
        );
        Some(response(c, url, username, password, &cnonce))
    }
}
fn hash(algorithm: &str, value: &str) -> String {
    if algorithm.starts_with("SHA-256") {
        hex::encode(Sha256::digest(value.as_bytes()))
    } else {
        hex::encode(Md5::digest(value.as_bytes()))
    }
}
fn quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
fn response(c: &Challenge, url: &Url, user: &str, password: &str, cnonce: &str) -> String {
    let uri = match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_string(),
    };
    let mut a1 = hash(&c.algorithm, &format!("{user}:{}:{password}", c.realm));
    if c.algorithm.ends_with("-SESS") {
        a1 = hash(&c.algorithm, &format!("{a1}:{}:{cnonce}", c.nonce));
    }
    let a2 = hash(&c.algorithm, &format!("GET:{uri}"));
    let nc = format!("{:08x}", c.nc);
    let digest = hash(
        &c.algorithm,
        &format!("{a1}:{}:{nc}:{cnonce}:auth:{a2}", c.nonce),
    );
    let mut value = format!("Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", algorithm={}, response=\"{digest}\", qop=auth, nc={nc}, cnonce=\"{cnonce}\"", quote(user), quote(&c.realm), quote(&c.nonce), quote(&uri), c.algorithm);
    if let Some(o) = &c.opaque {
        value.push_str(&format!(", opaque=\"{}\"", quote(o)));
    }
    value
}
fn fields(input: &str) -> Option<HashMap<String, String>> {
    let mut out = HashMap::new();
    let mut rest = input.trim();
    while !rest.is_empty() {
        let (name, tail) = rest.split_once('=')?;
        let tail = tail.trim_start();
        let (value, consumed) = if let Some(tail) = tail.strip_prefix('"') {
            let mut value = String::new();
            let mut escaped = false;
            let mut end = None;
            for (i, c) in tail.char_indices() {
                if escaped {
                    value.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    end = Some(i + 2);
                    break;
                } else {
                    value.push(c);
                }
            }
            (value, end?)
        } else {
            let n = tail.find(',').unwrap_or(tail.len());
            (tail[..n].trim().into(), n)
        };
        if out
            .insert(name.trim().to_ascii_lowercase(), value)
            .is_some()
        {
            return None;
        }
        rest = tail[consumed..].trim_start();
        if !rest.is_empty() {
            rest = rest.strip_prefix(',')?.trim_start();
        }
    }
    Some(out)
}

pub(crate) fn is_stale(challenge: &str) -> bool {
    challenge
        .split_once(' ')
        .and_then(|(_, rest)| fields(rest))
        .and_then(|f| f.get("stale").cloned())
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
}
