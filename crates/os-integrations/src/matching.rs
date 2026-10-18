//! Abgleich ohne Summierung; Query und Fragment bleiben Teil der Identität.
use os_model::config::UrlMatchConfig;
use url::Url;

pub fn match_key(address: &str, options: &UrlMatchConfig) -> Option<String> {
    let mut u = Url::parse(address).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    // Standardports werden bereits vom URL-Parser entfernt, explizite andere Ports bleiben.
    let _ = u.set_scheme("https");
    let mut path = normalize_percent(u.path());
    if options.ignore_trailing_slash && path != "/" {
        path = path.trim_end_matches('/').to_string();
    }
    let key = format!(
        "{}{}{}{}",
        &u[url::Position::BeforeHost..url::Position::BeforePath],
        path,
        u.query()
            .map(|q| format!("?{}", normalize_percent(q)))
            .unwrap_or_default(),
        u.fragment()
            .map(|q| format!("#{}", normalize_percent(q)))
            .unwrap_or_default()
    );
    Some(if options.ignore_case {
        key.to_lowercase()
    } else {
        key
    })
}
fn normalize_percent(s: &str) -> String {
    // Unreserved ASCII and UTF-8 werden normalisiert; kodierte Trenner bleiben kodiert.
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                if v.is_ascii_alphanumeric() || b"-._~".contains(&v) || v >= 128 {
                    out.push(v);
                } else {
                    out.extend_from_slice(format!("%{v:02X}").as_bytes());
                }
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
pub fn best_match<'a, T>(
    address: &str,
    rows: &'a [T],
    options: &UrlMatchConfig,
    row_url: impl Fn(&T) -> &str,
    rank: impl Fn(&T) -> (f64, f64),
) -> Option<&'a T> {
    let key = match_key(address, options)?;
    rows.iter()
        .filter(|r| match_key(row_url(r), options).as_ref() == Some(&key))
        .max_by(|a, b| {
            let (a1, a2) = rank(a);
            let (b1, b2) = rank(b);
            a1.total_cmp(&b1).then(a2.total_cmp(&b2))
        })
}
