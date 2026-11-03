//! Bereinigung der Texte eines Absturzberichts (OUT-246, Abschnitt Datenschutz).
//!
//! Reihenfolge: Geheimnisse wie OUT-243 → URLs (`http`, `https`, `ftp`, `file`) → E-Mail-Adressen → Hostnamen
//! dieser URLs auch ohne Schema → Heimatordner und `OPENSPIDER_HOME` → Dateinamen von Crawls und Exporten →
//! IPv4/IPv6-Adressen.
//! Die Funktion ist rein (keine Ein-/Ausgabe) und darf im Panic-Hook laufen: sie panikt nicht.
//! Wird von der CLI über `#[path]` mitkompiliert; deshalb keine Typen aus `os-app`.

use std::path::{Path, PathBuf};

/// Ersatztexte.
pub const URL: &str = "<url>";
pub const EMAIL: &str = "<email>";
pub const IP: &str = "<ip>";
pub const HOME: &str = "~";
pub const FILE: &str = "<file>";

/// Dateiendungen von Crawls, Konfigurationen und Exporten; deren Dateinamen kommen nicht in einen Bericht.
const DATA_EXTENSIONS: &[&str] = &[
    "ospider", "osconfig", "ospreset", "ostask", "csv", "tsv", "xlsx", "xls", "ods", "pdf", "xml",
    "gz", "zip", "sqlite", "db",
];

/// Bereinigung mit den Heimatpfaden dieses Prozesses.
#[derive(Debug, Clone, Default)]
pub struct Sanitizer {
    /// Schreibweisen der Heimatpfade, längste zuerst.
    homes: Vec<String>,
    /// Hostnamen aus URLs desselben Berichts; auch ohne Schema (z. B. in Debug-Zeilen von HTTP-Bibliotheken) → `<url>`.
    hosts: Vec<String>,
}

impl Sanitizer {
    /// `homes`: Benutzer-Heimatordner und `OPENSPIDER_HOME` (sofern gesetzt). Kanonische Formen und die
    /// Schreibweisen mit `/`, `\` und JSON-verdoppeltem `\\` werden mit ersetzt.
    pub fn new<I: IntoIterator<Item = PathBuf>>(homes: I) -> Self {
        let mut out: Vec<String> = Vec::new();
        for home in homes {
            let mut forms = vec![home.clone()];
            if let Ok(canonical) = std::fs::canonicalize(&home) {
                forms.push(canonical);
            }
            for path in forms {
                let raw = path_text(&path);
                let raw = raw.trim_end_matches(['/', '\\']).to_string();
                if raw
                    .chars()
                    .filter(|c| !matches!(c, '/' | '\\' | '.'))
                    .count()
                    < 2
                {
                    continue;
                }
                for form in [
                    raw.clone(),
                    raw.replace('\\', "/"),
                    raw.replace('/', "\\"),
                    raw.replace('\\', "\\\\"),
                ] {
                    if !out.contains(&form) {
                        out.push(form);
                    }
                }
            }
        }
        out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        Sanitizer {
            homes: out,
            hosts: Vec::new(),
        }
    }

    /// Dieselbe Bereinigung, zusätzlich mit den Hostnamen aller URLs in `texts`.
    pub fn with_hosts_from<'a, I: IntoIterator<Item = &'a str>>(&self, texts: I) -> Sanitizer {
        let mut hosts = self.hosts.clone();
        for text in texts {
            for host in url_hosts(text) {
                if !hosts.contains(&host) {
                    hosts.push(host);
                }
            }
        }
        hosts.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        Sanitizer {
            homes: self.homes.clone(),
            hosts,
        }
    }

    /// Bereinigt einen Text (mehrzeilig erlaubt).
    pub fn clean(&self, text: &str) -> String {
        let text = super::redact_secrets(text);
        let text = mask_urls(&text);
        let text = mask_emails(&text);
        let text = self.mask_hosts(&text);
        let text = self.mask_homes(&text);
        let text = mask_data_files(&text);
        mask_ips(&text)
    }

    fn mask_hosts(&self, text: &str) -> String {
        let mut text = text.to_string();
        for host in &self.hosts {
            text = replace_host(&text, host);
        }
        text
    }

    fn mask_homes(&self, text: &str) -> String {
        let mut text = text.to_string();
        for home in &self.homes {
            text = replace_path_prefix(&text, home);
        }
        text
    }
}

fn path_text(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    // Windows: `\\?\C:\…` aus canonicalize
    text.strip_prefix(r"\\?\")
        .map(str::to_string)
        .unwrap_or(text)
}

fn eq_path_chars(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Ersetzt `home` durch `~`, wenn danach kein weiteres Namenszeichen folgt (`/home/ab` ≠ `/home/abc`).
fn replace_path_prefix(text: &str, home: &str) -> String {
    if home.is_empty() || text.len() < home.len() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.len() >= home.len()
            && rest.is_char_boundary(home.len())
            && eq_path_chars(&rest[..home.len()], home)
        {
            let next = rest[home.len()..].chars().next();
            if !next.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.')) {
                out.push_str(HOME);
                i += home.len();
                continue;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

const SCHEMES: &[&str] = &["https://", "http://", "ftp://", "file:/"];

/// Hostnamen der URLs im Text (klein geschrieben, ohne Benutzerdaten und Port).
pub fn url_hosts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text.to_ascii_lowercase();
    for scheme in ["https://", "http://", "ftp://"] {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(scheme) {
            let start = from + pos + scheme.len();
            from = start;
            let rest = &lower[start..];
            let end = rest
                .find(|c: char| {
                    c.is_whitespace()
                        || matches!(
                            c,
                            '/' | '?' | '#' | '"' | '\'' | '`' | '>' | '\\' | '|' | ')'
                        )
                })
                .unwrap_or(rest.len());
            let authority = &rest[..end];
            let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            let host = if host_port.starts_with('[') {
                host_port
                    .split(']')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('[')
            } else {
                host_port.split(':').next().unwrap_or("")
            };
            let host = host.trim_end_matches('.');
            if host.len() >= 3
                && host.chars().any(|c| c.is_ascii_alphabetic())
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
                && !out.iter().any(|h: &String| h == host)
            {
                out.push(host.to_string());
            }
        }
    }
    out
}

/// Ersetzt einen Hostnamen (ohne Groß-/Kleinschreibung) an Namensgrenzen durch `<url>`.
fn replace_host(text: &str, host: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let is_name = |c: char| c.is_ascii_alphanumeric() || c == '-';
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(pos) = lower[from..].find(host) {
        let start = from + pos;
        let end = start + host.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let after_dot_name =
            text[end..].starts_with('.') && text[end + 1..].chars().next().is_some_and(is_name);
        // Davor darf ein Punkt stehen (Subdomain), danach weder ein Namenszeichen noch „.name“ (andere Domain).
        if before.is_none_or(|c| !is_name(c))
            && after.is_none_or(|c| !is_name(c))
            && !after_dot_name
        {
            out.push_str(&text[copied..start]);
            out.push_str(URL);
            copied = end;
        }
        from = end;
    }
    out.push_str(&text[copied..]);
    out
}

fn starts_with_ci(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// URLs der genannten Schemata → `<url>`; bereits maskierte Teile (`<redacted>`) gehören zur URL.
pub fn mask_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let boundary = text[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
        if boundary {
            if let Some(scheme) = SCHEMES.iter().find(|s| starts_with_ci(rest, s)) {
                let mut end = scheme.len();
                loop {
                    let tail = &rest[end..];
                    if tail.starts_with("<redacted>") {
                        end += "<redacted>".len();
                        continue;
                    }
                    match tail.chars().next() {
                        Some(c)
                            if !c.is_whitespace()
                                && !matches!(c, '"' | '\'' | '`' | '<' | '>' | '\\' | '|') =>
                        {
                            end += c.len_utf8();
                        }
                        _ => break,
                    }
                }
                // Satzzeichen am Ende gehören nicht zur URL.
                let mut url_end = end;
                while url_end > scheme.len() {
                    let last = rest[..url_end].chars().next_back().unwrap_or(' ');
                    if matches!(last, '.' | ',' | ';' | ':' | ')' | ']' | '}' | '!' | '?') {
                        url_end -= last.len_utf8();
                    } else {
                        break;
                    }
                }
                out.push_str(URL);
                out.push_str(&rest[url_end..end]);
                i += end;
                continue;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn is_email_local(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-')
}

fn is_email_domain(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-')
}

/// E-Mail-Adressen → `<email>`.
pub fn mask_emails(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        let mut start = i;
        while start > copied && is_email_local(bytes[start - 1] as char) {
            start -= 1;
        }
        // Lokaler Teil beginnt nicht mit einem Punkt.
        while start < i && bytes[start] == b'.' {
            start += 1;
        }
        let mut end = i + 1;
        while end < bytes.len() && is_email_domain(bytes[end] as char) {
            end += 1;
        }
        while end > i + 1 && matches!(bytes[end - 1], b'.' | b'-') {
            end -= 1;
        }
        let domain = &text[i + 1..end];
        let tld_ok = domain.rsplit_once('.').is_some_and(|(host, tld)| {
            !host.is_empty() && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())
        });
        if start < i && tld_ok {
            out.push_str(&text[copied..start]);
            out.push_str(EMAIL);
            copied = end;
            i = end;
        } else {
            i += 1;
        }
    }
    out.push_str(&text[copied..]);
    out
}

fn ipv4_at(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    for part in 0..4 {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() && i - start < 4 {
            i += 1;
        }
        let digits = &text[start..i];
        if digits.is_empty() || digits.len() > 3 || digits.parse::<u16>().ok()? > 255 {
            return None;
        }
        if part < 3 {
            if bytes.get(i) != Some(&b'.') {
                return None;
            }
            i += 1;
        }
    }
    // Danach keine weitere Ziffer, kein Buchstabe und kein „.Ziffer“ (Versionen wie 1.2.3.4.5).
    match bytes.get(i) {
        Some(c) if c.is_ascii_alphanumeric() || *c == b'_' => None,
        Some(b'.') if bytes.get(i + 1).is_some_and(|c| c.is_ascii_digit()) => None,
        _ => Some(i),
    }
}

fn valid_ipv6(candidate: &str) -> bool {
    if candidate.matches(':').count() < 2 || !candidate.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let compressed = candidate.contains("::");
    if candidate.matches("::").count() > 1 || candidate.contains(":::") {
        return false;
    }
    let groups: Vec<&str> = candidate.split(':').collect();
    let mut count = 0;
    for (n, group) in groups.iter().enumerate() {
        if group.is_empty() {
            continue;
        }
        if n == groups.len() - 1 && group.contains('.') {
            if ipv4_at(group) != Some(group.len()) {
                return false;
            }
            count += 2;
            continue;
        }
        if group.len() > 4 || !group.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
        count += 1;
    }
    if compressed {
        count <= 7
    } else {
        count == 8
    }
}

/// IPv4- und IPv6-Adressen → `<ip>`.
pub fn mask_ips(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        let prev = if i == 0 { None } else { Some(bytes[i - 1]) };
        let boundary =
            prev.is_none_or(|c| !(c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b':')));
        if boundary && (bytes[i].is_ascii_hexdigit() || bytes[i] == b':') {
            // IPv6: längster Lauf aus Hexziffern, ':' und '.'
            let mut end = i;
            while end < bytes.len()
                && (bytes[end].is_ascii_hexdigit() || matches!(bytes[end], b':' | b'.'))
            {
                end += 1;
            }
            let mut candidate = &text[i..end];
            while candidate.ends_with('.')
                || (candidate.ends_with(':') && !candidate.ends_with("::"))
            {
                candidate = &candidate[..candidate.len() - 1];
            }
            let after = bytes.get(i + candidate.len());
            let clean_end = after.is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
            if clean_end && valid_ipv6(candidate) {
                out.push_str(&text[copied..i]);
                out.push_str(IP);
                i += candidate.len();
                copied = i;
                continue;
            }
            if bytes[i].is_ascii_digit() {
                if let Some(len) = ipv4_at(&text[i..]) {
                    out.push_str(&text[copied..i]);
                    out.push_str(IP);
                    i += len;
                    copied = i;
                    continue;
                }
            }
        }
        i += 1;
        while i < bytes.len() && !text.is_char_boundary(i) {
            i += 1;
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Dateinamen mit Endungen von Crawls, Konfigurationen und Exporten → `<file>` (Ordner bleiben erhalten).
pub fn mask_data_files(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'.' {
            i += 1;
            continue;
        }
        let ext_end = {
            let mut e = i + 1;
            while e < bytes.len() && bytes[e].is_ascii_alphanumeric() {
                e += 1;
            }
            e
        };
        let ext = &text[i + 1..ext_end];
        let next_ok = bytes
            .get(ext_end)
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
        if ext.is_empty()
            || !next_ok
            || !DATA_EXTENSIONS.iter().any(|d| d.eq_ignore_ascii_case(ext))
        {
            i = ext_end.max(i + 1);
            continue;
        }
        // Anfang des Dateinamens: bis zum Pfadtrenner, Leerraum oder Anführungszeichen.
        let mut start = i;
        while start > copied {
            let c = text[..start].chars().next_back().unwrap_or(' ');
            if c.is_whitespace()
                || matches!(
                    c,
                    '/' | '\\' | '"' | '\'' | '`' | '=' | '(' | '[' | '<' | '>' | ':'
                )
            {
                break;
            }
            start -= c.len_utf8();
        }
        if start == i {
            i = ext_end;
            continue;
        }
        out.push_str(&text[copied..start]);
        out.push_str(FILE);
        copied = ext_end;
        i = ext_end;
    }
    out.push_str(&text[copied..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s() -> Sanitizer {
        Sanitizer::new([PathBuf::from("/home/alice"), PathBuf::from("/srv/os-home")])
    }

    // spec: OUT-246
    #[test]
    fn urls_of_all_schemes_become_placeholders() {
        assert_eq!(
            mask_urls("GET https://crash-secret.test/a?token=x done"),
            "GET <url> done"
        );
        assert_eq!(mask_urls("(http://a.test/b)."), "(<url>).");
        assert_eq!(
            mask_urls("FTP://files.test/x, FILE:///etc/x"),
            "<url>, <url>"
        );
        assert_eq!(
            mask_urls("url=\"https://<redacted>@crash-secret.test/?token=<redacted>\""),
            "url=\"<url>\""
        );
        assert_eq!(mask_urls("ws://keep.test x"), "ws://keep.test x");
        assert_eq!(mask_urls("xhttp://no.test"), "xhttp://no.test");
        assert_eq!(mask_urls("ä https://ü.test/ö ä"), "ä <url> ä");
    }

    // spec: OUT-246
    #[test]
    fn emails_become_placeholders() {
        assert_eq!(
            mask_emails("mail ops-246@crash-secret.test."),
            "mail <email>."
        );
        assert_eq!(mask_emails("a.b+c@x.example.com,"), "<email>,");
        assert_eq!(mask_emails("<redacted>@host.test"), "<redacted>@host.test");
        assert_eq!(
            mask_emails("version@2 and user@localhost"),
            "version@2 and user@localhost"
        );
    }

    // spec: OUT-246
    #[test]
    fn ipv4_and_ipv6_become_placeholders_but_not_versions_paths_or_times() {
        assert_eq!(mask_ips("peer 192.168.0.10:443 ok"), "peer <ip>:443 ok");
        assert_eq!(
            mask_ips("v1.2.3.4.5 and 256.1.1.1"),
            "v1.2.3.4.5 and 256.1.1.1"
        );
        assert_eq!(
            mask_ips("[::1]:8080 fe80::1ff:fe23:4567:890a"),
            "[<ip>]:8080 <ip>"
        );
        assert_eq!(mask_ips("2001:db8:0:0:0:0:2:1 x"), "<ip> x");
        assert_eq!(mask_ips("::ffff:10.0.0.1"), "<ip>");
        assert_eq!(
            mask_ips("os_app::crash::core at 10:15:30.123+01:00 ab::cd de::Error"),
            "os_app::crash::core at 10:15:30.123+01:00 ab::cd de::Error"
        );
        assert_eq!(mask_ips("mac aa:bb:cc:dd:ee:ff"), "mac aa:bb:cc:dd:ee:ff");
    }

    // spec: OUT-246
    #[test]
    fn home_and_openspider_home_become_tilde_in_all_spellings() {
        let s = s();
        assert_eq!(s.clean("at /home/alice/src/x.rs:3"), "at ~/src/x.rs:3");
        assert_eq!(s.clean("/home/alicex/keep"), "/home/alicex/keep");
        assert_eq!(s.clean("logs in /srv/os-home/logs"), "logs in ~/logs");
        let win = Sanitizer::new([PathBuf::from(r"C:\Users\Bob")]);
        assert_eq!(win.clean(r"C:\Users\Bob\AppData x"), r"~\AppData x");
        assert_eq!(win.clean(r#"{"p":"C:\\Users\\Bob\\x"}"#), r#"{"p":"~\\x"}"#);
        assert_eq!(win.clean("C:/Users/Bob/x"), "~/x");
    }

    // spec: OUT-246
    // spec: OUT-243 [M3]
    #[test]
    fn secrets_are_redacted_like_logs() {
        let s = s();
        let text = "Authorization: Bearer SEKRET-1\npassword=SEKRET-2 Cookie: a=SEKRET-3";
        let clean = s.clean(text);
        assert!(!clean.contains("SEKRET"), "{clean}");
        assert!(clean.contains("<redacted>"));
        assert_eq!(clean.lines().count(), 2);
    }

    // spec: OUT-246
    #[test]
    fn crawl_and_export_file_names_are_removed() {
        let s = s();
        assert_eq!(
            s.clean("saved /home/alice/Crawls/kunde-x.ospider and C:\\exp\\Bericht Q3.xlsx"),
            "saved ~/Crawls/<file> and C:\\exp\\Bericht <file>"
        );
        assert_eq!(
            s.clean("file=\"export.csv\" openspider.log"),
            "file=\"<file>\" openspider.log"
        );
    }

    // spec: OUT-246
    #[test]
    fn hosts_of_urls_are_masked_also_without_scheme() {
        let lines = [
            "DEBUG Abruf url=\"http://user:pw@Crash-Secret.test:8080/a\"",
            "DEBUG starting new connection 'Some(\"crash-secret.test\")'",
            "pooling idle connection for (\"http\", crash-secret.test) sub.crash-secret.test.example",
            "keep crash-secret.testing and xcrash-secret.test",
            "www.crash-secret.test/ok",
        ];
        assert_eq!(url_hosts(lines[0]), ["crash-secret.test"]);
        let s = s().with_hosts_from(lines);
        let clean: Vec<String> = lines.iter().map(|l| s.clean(l)).collect();
        assert_eq!(clean[0], "DEBUG Abruf url=\"<url>\"");
        assert_eq!(clean[1], "DEBUG starting new connection 'Some(\"<url>\")'");
        assert_eq!(
            clean[2],
            "pooling idle connection for (\"http\", <url>) sub.crash-secret.test.example"
        );
        assert_eq!(clean[3], "keep crash-secret.testing and xcrash-secret.test");
        assert_eq!(clean[4], "www.<url>/ok");
        assert!(url_hosts("http://127.0.0.1:9/ and https://[::1]/").is_empty());
    }

    // spec: OUT-246
    #[test]
    fn combined_fixture_leaves_no_trace() {
        let s = s();
        let text = "INFO fetch https://user:pw-SEKRET-246@crash-secret.test/?token=SEKRET-246 from 10.1.2.3 \
                    by ops-246@crash-secret.test home=/srv/os-home/crawls/crash-secret.test.ospider";
        let clean = s.clean(text);
        for needle in [
            "crash-secret.test",
            "SEKRET-246",
            "ops-246",
            "/srv/os-home",
            "10.1.2.3",
        ] {
            assert!(!clean.contains(needle), "{needle} in {clean}");
        }
        assert!(clean.contains("<url>") && clean.contains('~') && clean.contains("<ip>"));
    }
}
