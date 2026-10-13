//! Eigene Dekompression des Antwort-Bodys (CRAWL-011).
//!
//! reqwest läuft ohne automatische Dekompression, damit die über die Leitung gelaufenen Bytes
//! (`Übertragen`) exakt gezählt werden. Unterstützt: `gzip`/`x-gzip`, `deflate` (zlib und raw),
//! `br` sowie Ketten wie `gzip, br`. Defekte Daten führen zu einem Fehlertext, nie zu einem Absturz.

use std::io::Read;

/// Ein `Content-Encoding`-Wert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encoding {
    /// `identity` bzw. leer.
    Identity,
    /// `gzip`, `x-gzip`.
    Gzip,
    /// `deflate` (zlib-umhüllt oder roh).
    Deflate,
    /// `br`.
    Brotli,
    /// Nicht unterstützt (z. B. `zstd`, `compress`).
    Unknown(String),
}

/// Zerlegt die `Content-Encoding`-Header in die Reihenfolge, in der die Server sie angewendet haben.
pub fn parse_encodings<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<Encoding> {
    values
        .into_iter()
        .flat_map(|v| v.split(','))
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .map(|t| match t.as_str() {
            "identity" => Encoding::Identity,
            "gzip" | "x-gzip" => Encoding::Gzip,
            "deflate" => Encoding::Deflate,
            "br" => Encoding::Brotli,
            _ => Encoding::Unknown(t),
        })
        .filter(|e| *e != Encoding::Identity)
        .collect()
}

/// Ergebnis der Dekompression.
#[derive(Debug, Default)]
pub struct Decoded {
    /// Dekomprimierter Body, höchstens `max_out` Bytes.
    pub body: Vec<u8>,
    /// Die Ausgabe wurde bei `max_out` abgeschnitten.
    pub truncated: bool,
    /// Detailtext, wenn die Daten nicht (vollständig) dekodierbar waren.
    pub error: Option<String>,
}

/// Dekomprimiert `raw` mit den angegebenen Kodierungen (letzte zuerst entpacken).
///
/// `input_truncated`: der komprimierte Body wurde selbst gekürzt, ein vorzeitiges Ende des Stroms ist
/// dann kein Fehler.
pub fn decode(
    raw: Vec<u8>,
    encodings: &[Encoding],
    max_out: u64,
    input_truncated: bool,
) -> Decoded {
    if encodings.is_empty() {
        let mut d = Decoded {
            body: raw,
            ..Decoded::default()
        };
        if d.body.len() as u64 > max_out {
            d.body.truncate(max_out as usize);
            d.truncated = true;
        }
        return d;
    }
    if raw.is_empty() {
        // 204/304/HEAD-artige Antworten: nichts zu dekodieren, auch wenn der Header gesetzt ist.
        return Decoded::default();
    }
    let mut data = raw;
    let mut truncated = input_truncated;
    for enc in encodings.iter().rev() {
        let (out, exceeded, error) = decode_layer(enc, &data, max_out);
        truncated |= exceeded;
        if let Some(e) = error {
            if !(input_truncated || exceeded) || out.is_empty() {
                return Decoded {
                    body: out,
                    truncated,
                    error: Some(e),
                };
            }
        }
        data = out;
    }
    if data.len() as u64 > max_out {
        data.truncate(max_out as usize);
        truncated = true;
    }
    Decoded {
        body: data,
        truncated,
        error: None,
    }
}

/// Entpackt eine Schicht; liefert (Ausgabe, Grenze erreicht, Fehlertext).
fn decode_layer(enc: &Encoding, input: &[u8], max_out: u64) -> (Vec<u8>, bool, Option<String>) {
    match enc {
        Encoding::Gzip => read_limited(flate2::read::MultiGzDecoder::new(input), max_out, "gzip"),
        Encoding::Deflate => decode_deflate(input, max_out),
        Encoding::Brotli => read_limited(brotli::Decompressor::new(input, 8192), max_out, "br"),
        Encoding::Identity => (input.to_vec(), false, None),
        Encoding::Unknown(name) => (
            Vec::new(),
            false,
            Some(format!("nicht unterstütztes Content-Encoding {name:?}")),
        ),
    }
}

/// `deflate` ist in der Praxis mal zlib-umhüllt (RFC 1950, korrekt), mal roh (RFC 1951): erst nach dem
/// Zlib-Kopf entscheiden, bei Fehlschlag ohne Ausgabe die andere Variante versuchen.
fn decode_deflate(input: &[u8], max_out: u64) -> (Vec<u8>, bool, Option<String>) {
    let looks_zlib = input.len() >= 2
        && input[0] & 0x0f == 8
        && ((u16::from(input[0]) << 8) | u16::from(input[1])) % 31 == 0;
    let zlib =
        |i: &[u8]| read_limited(flate2::read::ZlibDecoder::new(i), max_out, "deflate (zlib)");
    let raw = |i: &[u8]| {
        read_limited(
            flate2::read::DeflateDecoder::new(i),
            max_out,
            "deflate (raw)",
        )
    };
    let first = if looks_zlib { zlib(input) } else { raw(input) };
    if first.2.is_none() || !first.0.is_empty() {
        return first;
    }
    let second = if looks_zlib { raw(input) } else { zlib(input) };
    if second.2.is_none() {
        second
    } else {
        first
    }
}

fn read_limited<R: Read>(reader: R, max_out: u64, label: &str) -> (Vec<u8>, bool, Option<String>) {
    let mut out = Vec::new();
    let limit = max_out.saturating_add(1);
    let result = reader.take(limit).read_to_end(&mut out);
    let exceeded = out.len() as u64 > max_out;
    if exceeded {
        out.truncate(max_out as usize);
    }
    let error = result
        .err()
        .map(|e| format!("{label}: Daten nicht dekodierbar ({e})"));
    (out, exceeded, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const TEXT: &[u8] = b"<html><body>Hallo Welt, Hallo Welt, Hallo Welt, Hallo Welt</body></html>";

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }
    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }
    fn raw_deflate(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }
    fn brotli_enc(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut w = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
            w.write_all(data).unwrap();
        }
        out
    }

    // spec: CRAWL-011
    #[test]
    fn parses_encoding_lists() {
        assert_eq!(parse_encodings(["gzip"]), vec![Encoding::Gzip]);
        assert_eq!(
            parse_encodings(["GZIP, br", "deflate"]),
            vec![Encoding::Gzip, Encoding::Brotli, Encoding::Deflate]
        );
        assert_eq!(parse_encodings(["identity", ""]), Vec::<Encoding>::new());
        assert_eq!(parse_encodings(["x-gzip"]), vec![Encoding::Gzip]);
        assert_eq!(
            parse_encodings(["zstd"]),
            vec![Encoding::Unknown("zstd".into())]
        );
    }

    // spec: CRAWL-011
    #[test]
    fn gzip_zlib_raw_deflate_and_brotli_roundtrip() {
        for (enc, data) in [
            (Encoding::Gzip, gzip(TEXT)),
            (Encoding::Deflate, zlib(TEXT)),
            (Encoding::Deflate, raw_deflate(TEXT)),
            (Encoding::Brotli, brotli_enc(TEXT)),
        ] {
            let d = decode(data, std::slice::from_ref(&enc), 1 << 20, false);
            assert_eq!(d.error, None, "{enc:?}");
            assert_eq!(d.body, TEXT, "{enc:?}");
            assert!(!d.truncated);
        }
    }

    // spec: CRAWL-011
    #[test]
    fn chained_encodings_are_unwrapped_in_reverse_order() {
        // Server: erst gzip, dann br  ->  `Content-Encoding: gzip, br`
        let data = brotli_enc(&gzip(TEXT));
        let d = decode(data, &[Encoding::Gzip, Encoding::Brotli], 1 << 20, false);
        assert_eq!(d.error, None);
        assert_eq!(d.body, TEXT);
    }

    // spec: CRAWL-011
    #[test]
    fn broken_data_reports_an_error_without_panicking() {
        for enc in [Encoding::Gzip, Encoding::Deflate, Encoding::Brotli] {
            let d = decode(
                b"das ist gar nicht komprimiert".to_vec(),
                std::slice::from_ref(&enc),
                1 << 20,
                false,
            );
            assert!(d.error.is_some(), "{enc:?}");
        }
        let d = decode(
            vec![1, 2, 3],
            &[Encoding::Unknown("zstd".into())],
            100,
            false,
        );
        assert!(d.error.unwrap().contains("zstd"));
    }

    // spec: CRAWL-011
    #[test]
    fn truncated_stream_is_an_error_unless_the_input_was_cut_by_us() {
        let full = gzip(&TEXT.repeat(50));
        let cut = full[..full.len() / 2].to_vec();
        let broken = decode(cut.clone(), &[Encoding::Gzip], 1 << 20, false);
        assert!(broken.error.is_some());
        let tolerated = decode(cut, &[Encoding::Gzip], 1 << 20, true);
        assert_eq!(tolerated.error, None);
        assert!(tolerated.truncated);
        assert!(!tolerated.body.is_empty());
    }

    // spec: CRAWL-011
    #[test]
    fn output_is_capped_against_decompression_bombs() {
        let big = vec![b'a'; 1_000_000];
        let d = decode(gzip(&big), &[Encoding::Gzip], 1000, false);
        assert_eq!(d.error, None);
        assert!(d.truncated);
        assert_eq!(d.body.len(), 1000);
    }

    // spec: CRAWL-011
    #[test]
    fn empty_body_with_encoding_header_is_fine() {
        let d = decode(Vec::new(), &[Encoding::Gzip], 100, false);
        assert_eq!(d.error, None);
        assert!(d.body.is_empty());
    }

    // spec: CRAWL-011
    #[test]
    fn identity_is_only_capped() {
        let d = decode(vec![7; 50], &[], 10, false);
        assert_eq!(d.body.len(), 10);
        assert!(d.truncated);
        let d = decode(vec![7; 10], &[], 10, false);
        assert!(!d.truncated);
    }
}
