//! Dateinamen der Exporte (OUT-008).
//!
//! STUB(M1-P08)

/// Dateiname zu einem kanonischen Export-Namen, optional mit Segment (`_segment_<name>`).
// STUB(M1-P08): Stub hängt nur die Endung an.
pub fn file_name(canonical: &str, segment: Option<&str>, ext: &str) -> String {
    match segment {
        Some(s) => format!("{canonical}_segment_{s}.{ext}"),
        None => format!("{canonical}.{ext}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_extension() {
        assert_eq!(file_name("internal_all", None, "csv"), "internal_all.csv");
    }
}
