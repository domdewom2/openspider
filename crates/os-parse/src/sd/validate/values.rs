//! Syntax der Literalwerte (TECH-187 `value_format`, TECH-188 `invalid_value`): ISO-8601-Datum, -Zeit und -Dauer,
//! Zahlen, Wahrheitswerte, Währungscodes, URLs.

use serde_json::Value;

/// Zahl: JSON-Zahl oder Text wie `5`, `-1.25`, `3e2` (Dezimalpunkt, keine Tausendertrennung).
pub fn is_number(v: &Value) -> bool {
    match v {
        Value::Number(_) => true,
        Value::String(s) => number_text(s.trim()),
        _ => false,
    }
}

fn number_text(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (mantissa, exponent) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (s, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = digits(int) && digits(frac) && !(int.is_empty() && frac.is_empty());
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exponent_ok
}

/// Ganzzahl: JSON-Ganzzahl oder Ziffernfolge mit optionalem Vorzeichen.
pub fn is_integer(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.is_i64() || n.is_u64(),
        Value::String(s) => {
            let s = s.trim();
            let s = s.strip_prefix(['+', '-']).unwrap_or(s);
            !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
        }
        _ => false,
    }
}

/// Wahrheitswert: `true`/`false` (auch als Text) oder `https://schema.org/True|False`.
pub fn is_boolean(v: &Value) -> bool {
    match v {
        Value::Bool(_) => true,
        Value::String(s) => {
            let s = s.trim();
            s.eq_ignore_ascii_case("true")
                || s.eq_ignore_ascii_case("false")
                || ["https://schema.org/", "http://schema.org/"]
                    .iter()
                    .any(|p| {
                        s.strip_prefix(p)
                            .is_some_and(|r| r == "True" || r == "False")
                    })
        }
        _ => false,
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn digits(s: &str, n: usize) -> Option<u32> {
    (s.len() == n && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

/// Kalenderdatum `JJJJ-MM-TT` mit gültigem Monat und Tag.
pub fn is_full_date(s: &str) -> bool {
    let mut parts = s.split('-');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    match (digits(y, 4), digits(m, 2), digits(d, 2)) {
        (Some(y), Some(m), Some(d)) => (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m),
        _ => false,
    }
}

/// Uhrzeit `hh:mm[:ss[.f]]` mit optionaler Zone (`Z`, `+01:00`, `-0500`).
pub fn is_time(s: &str) -> bool {
    let (clock, zone) = match s.find(['Z', 'z', '+', '-']) {
        Some(p) => (&s[..p], Some(&s[p..])),
        None => (s, None),
    };
    let zone_ok = zone.is_none_or(|z| {
        if z.eq_ignore_ascii_case("z") {
            return true;
        }
        let z = &z[1..];
        let (h, m) = z
            .split_once(':')
            .unwrap_or((z.get(..2).unwrap_or(""), z.get(2..).unwrap_or("")));
        digits(h, 2).is_some_and(|h| h <= 14)
            && (m.is_empty() || digits(m, 2).is_some_and(|m| m < 60))
    });
    let mut parts = clock.split(':');
    let (Some(h), Some(m)) = (parts.next(), parts.next()) else {
        return false;
    };
    let sec_ok = match (parts.next(), parts.next()) {
        (None, None) => true,
        (Some(sec), None) => {
            let (whole, frac) = sec.split_once('.').unwrap_or((sec, "0"));
            digits(whole, 2).is_some_and(|s| s <= 60)
                && !frac.is_empty()
                && frac.bytes().all(|b| b.is_ascii_digit())
        }
        _ => false,
    };
    digits(h, 2).is_some_and(|h| h <= 24)
        && digits(m, 2).is_some_and(|m| m < 60)
        && sec_ok
        && zone_ok
}

/// Datum mit Uhrzeit (`JJJJ-MM-TTThh:mm…`, Trenner `T` oder Leerzeichen).
pub fn is_datetime(s: &str) -> bool {
    s.split_once(['T', 't', ' '])
        .is_some_and(|(d, t)| is_full_date(d) && is_time(t))
}

/// ISO-8601-Datum in verkürzter oder voller Form (`JJJJ`, `JJJJ-MM`, `JJJJ-MM-TT`) oder Datum mit Uhrzeit.
pub fn is_iso_date(s: &str) -> bool {
    let s = s.trim();
    if is_full_date(s) || is_datetime(s) || digits(s, 4).is_some() {
        return true;
    }
    s.split_once('-').is_some_and(|(y, m)| {
        digits(y, 4).is_some() && digits(m, 2).is_some_and(|m| (1..=12).contains(&m))
    })
}

/// Dauer nach ISO 8601, z. B. `PT1H30M`, `P2D`, `PT45.5S`.
pub fn is_duration(s: &str) -> bool {
    let Some(rest) = s.trim().strip_prefix('P') else {
        return false;
    };
    let (date, time) = match rest.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (rest, None),
    };
    let Some(date_count) = duration_units(date, "YMWD") else {
        return false;
    };
    match time {
        Some(t) => duration_units(t, "HMS").is_some_and(|n| n > 0),
        None => date_count > 0,
    }
}

/// Zählt die Komponenten `<Zahl><Einheit>` in der Reihenfolge `order`; nur Sekunden dürfen einen Bruchteil haben.
fn duration_units(s: &str, order: &str) -> Option<usize> {
    let (mut count, mut rest, mut allowed) = (0, s, order);
    while !rest.is_empty() {
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
        let (number, tail) = rest.split_at(end);
        let unit = tail.chars().next()?;
        let pos = allowed.find(unit)?;
        let whole = !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit());
        if !(whole || (unit == 'S' && number_text(number))) {
            return None;
        }
        allowed = &allowed[pos + 1..];
        rest = &tail[unit.len_utf8()..];
        count += 1;
    }
    Some(count)
}

/// Währungscode aus drei Großbuchstaben (ISO 4217).
pub fn is_currency(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_uppercase())
}

/// Absolute HTTP(S)-URL oder relativer Verweis ohne Leerraum.
pub fn is_url(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.chars().any(char::is_whitespace) {
        return false;
    }
    match url::Url::parse(s) {
        Ok(u) => matches!(u.scheme(), "http" | "https") && u.host().is_some(),
        Err(url::ParseError::RelativeUrlWithoutBase) => true,
        Err(_) => false,
    }
}
