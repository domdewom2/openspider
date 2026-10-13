//! Cookie-Speicher nach RFC 6265 (CRAWL-088).
//!
//! Der Speicher gehört nicht dem Fetcher, sondern dem Aufrufer: Die Engine legt je Seed einen
//! Speicher an (`CookieMode::SessionOnly`) und reicht ihn an Weiterleitungsziele weiter, oder sie
//! teilt einen globalen Speicher über den ganzen Crawl (`CookieMode::Persistent`). Der Fetcher liest
//! daraus den `Cookie`-Header und schreibt die `Set-Cookie`-Header der Antwort hinein; im Modus
//! `None` fasst er den Speicher nie an. Domain-, Pfad-, Secure- und Ablaufregeln übernimmt
//! `cookie_store` (Secure-Cookies werden nicht über `http` gesendet).

use std::sync::{Mutex, MutexGuard};

use cookie_store::{CookieStore, RawCookie};
use url::Url;

/// Cookie-Speicher einer Crawl-Sitzung (Wrapper um `cookie_store`).
///
/// `Debug` zeigt nur die Anzahl, nie Namen oder Werte (OUT-243).
#[derive(Default)]
pub struct CookieJar {
    store: Mutex<CookieStore>,
}

impl std::fmt::Debug for CookieJar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieJar")
            .field("cookies", &self.len())
            .finish()
    }
}

impl CookieJar {
    /// Leerer Speicher.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, CookieStore> {
        self.store.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Wert des `Cookie`-Headers für eine Anfrage an `url` (`a=1; b=2`); `None`, wenn kein Cookie passt.
    pub fn header_for(&self, url: &Url) -> Option<String> {
        let store = self.lock();
        let pairs: Vec<String> = store
            .get_request_values(url)
            .map(|(n, v)| format!("{n}={v}"))
            .collect();
        if pairs.is_empty() {
            None
        } else {
            Some(pairs.join("; "))
        }
    }

    /// Speichert die `Set-Cookie`-Werte einer Antwort von `url`. Ungültige Cookies werden verworfen.
    /// Gibt die Zahl der gespeicherten Cookies zurück.
    pub fn store_set_cookie<'a>(
        &self,
        url: &Url,
        set_cookie_values: impl IntoIterator<Item = &'a str>,
    ) -> usize {
        let mut store = self.lock();
        let mut stored = 0;
        for value in set_cookie_values {
            let Ok(raw) = RawCookie::parse(value.to_string()) else {
                continue;
            };
            if store.insert_raw(&raw, url).is_ok() {
                stored += 1;
            }
        }
        stored
    }

    /// Spielt ein einzelnes Cookie (`name=wert; Path=/ …`) so ein, als hätte `url` es gesetzt
    /// (z. B. Cookies aus einem Formular-Login, CRAWL-120). `true`, wenn es gespeichert wurde.
    pub fn insert_raw(&self, url: &Url, set_cookie_value: &str) -> bool {
        self.store_set_cookie(url, [set_cookie_value]) == 1
    }

    /// Anzahl der nicht abgelaufenen Cookies.
    pub fn len(&self) -> usize {
        self.lock().iter_unexpired().count()
    }

    /// Ob der Speicher leer ist.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Verwirft alle Cookies (Beginn eines neuen Crawls).
    pub fn clear(&self) {
        self.lock().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    // spec: CRAWL-088
    #[test]
    fn set_cookie_is_returned_for_same_host() {
        let jar = CookieJar::new();
        assert!(jar.is_empty());
        assert_eq!(jar.header_for(&u("http://a.test/x")), None);
        assert_eq!(
            jar.store_set_cookie(&u("http://a.test/set"), ["sid=abc123; Path=/"]),
            1
        );
        assert_eq!(
            jar.header_for(&u("http://a.test/check")).as_deref(),
            Some("sid=abc123")
        );
        assert_eq!(jar.len(), 1);
    }

    // spec: CRAWL-088
    #[test]
    fn host_only_cookie_is_not_sent_to_other_hosts() {
        let jar = CookieJar::new();
        jar.store_set_cookie(&u("http://a.test/"), ["sid=1"]);
        assert_eq!(jar.header_for(&u("http://b.test/")), None);
        assert_eq!(jar.header_for(&u("http://sub.a.test/")), None);
    }

    // spec: CRAWL-088
    #[test]
    fn domain_cookie_reaches_subdomains() {
        let jar = CookieJar::new();
        jar.store_set_cookie(&u("http://www.site.test/"), ["d=1; Domain=site.test"]);
        assert_eq!(
            jar.header_for(&u("http://shop.site.test/")).as_deref(),
            Some("d=1")
        );
        // fremde Domain im Set-Cookie wird abgelehnt (RFC 6265, 5.3)
        assert_eq!(
            jar.store_set_cookie(&u("http://www.site.test/"), ["x=1; Domain=other.test"]),
            0
        );
    }

    // spec: CRAWL-088
    #[test]
    fn path_scope_is_respected() {
        let jar = CookieJar::new();
        jar.store_set_cookie(&u("http://a.test/app/login"), ["p=1; Path=/app"]);
        assert!(jar.header_for(&u("http://a.test/app/x")).is_some());
        assert!(jar.header_for(&u("http://a.test/other")).is_none());
        assert!(jar.header_for(&u("http://a.test/application")).is_none());
    }

    // spec: CRAWL-088
    #[test]
    fn secure_cookie_is_not_sent_over_http() {
        let jar = CookieJar::new();
        jar.store_set_cookie(&u("https://a.test/"), ["s=1; Secure; Path=/"]);
        assert_eq!(jar.header_for(&u("http://a.test/secure-only")), None);
        assert_eq!(
            jar.header_for(&u("https://a.test/secure-only")).as_deref(),
            Some("s=1")
        );
    }

    // spec: CRAWL-088
    #[test]
    fn expired_and_max_age_zero_cookies_are_dropped() {
        let jar = CookieJar::new();
        jar.store_set_cookie(&u("http://a.test/"), ["t=1; Path=/"]);
        assert!(jar.header_for(&u("http://a.test/")).is_some());
        // Löschen per Max-Age=0 bzw. Datum in der Vergangenheit
        jar.store_set_cookie(&u("http://a.test/"), ["t=; Path=/; Max-Age=0"]);
        assert_eq!(jar.header_for(&u("http://a.test/")), None);
        assert_eq!(
            jar.store_set_cookie(
                &u("http://a.test/"),
                ["old=1; Expires=Wed, 21 Oct 2015 07:28:00 GMT"]
            ),
            0
        );
    }

    // spec: CRAWL-088
    #[test]
    fn clear_empties_the_jar_and_debug_hides_values() {
        let jar = CookieJar::new();
        jar.store_set_cookie(&u("http://a.test/"), ["sid=top-secret-value"]);
        let dbg = format!("{jar:?}");
        assert!(!dbg.contains("top-secret-value"), "{dbg}");
        jar.clear();
        assert!(jar.is_empty());
        assert!(jar.insert_raw(&u("http://a.test/"), "login=1; Path=/"));
        assert_eq!(
            jar.header_for(&u("http://a.test/x")).as_deref(),
            Some("login=1")
        );
    }

    // spec: CRAWL-088
    #[test]
    fn garbage_set_cookie_is_ignored() {
        let jar = CookieJar::new();
        assert_eq!(
            jar.store_set_cookie(&u("http://a.test/"), ["", "=", ";;;"]),
            0
        );
        assert!(jar.is_empty());
    }
}
