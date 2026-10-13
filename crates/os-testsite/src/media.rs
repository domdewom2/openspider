//! Kodierung von Antwort-Bodies (`encoding`) und PNG-Erzeugung (`png_size`).

use std::io::Write;

use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::{Compression, Crc};
use serde::Deserialize;

/// Wert der Route-Option `encoding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Encoding {
    /// gzip (`Content-Encoding: gzip`).
    Gzip,
    /// zlib-umhülltes deflate, wie von HTTP für `Content-Encoding: deflate` vorgesehen.
    Deflate,
    /// Rohes deflate ohne zlib-Kopf (`Content-Encoding: deflate`, häufiger Server-Fehler).
    DeflateRaw,
    /// Brotli (`Content-Encoding: br`).
    Br,
    /// Gültiger gzip-Kopf, aber abgeschnittener Datenstrom (`Content-Encoding: gzip`).
    BrokenGzip,
}

impl Encoding {
    /// Wert des `Content-Encoding`-Headers.
    pub fn header_value(self) -> &'static str {
        match self {
            Encoding::Gzip | Encoding::BrokenGzip => "gzip",
            Encoding::Deflate | Encoding::DeflateRaw => "deflate",
            Encoding::Br => "br",
        }
    }

    /// Kodiert `data` deterministisch.
    pub fn encode(self, data: &[u8]) -> Vec<u8> {
        match self {
            Encoding::Gzip => gzip(data),
            Encoding::BrokenGzip => {
                let mut full = gzip(data);
                // Kopf (10 Byte) bleibt gültig, der Rest wird abgeschnitten (nie vollständig).
                let keep = (full.len() / 2).max(11).min(full.len().saturating_sub(1));
                full.truncate(keep);
                full
            }
            Encoding::Deflate => {
                let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
                let _ = enc.write_all(data);
                enc.finish().unwrap_or_default()
            }
            Encoding::DeflateRaw => {
                let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
                let _ = enc.write_all(data);
                enc.finish().unwrap_or_default()
            }
            Encoding::Br => {
                let mut out = Vec::new();
                {
                    let mut enc = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
                    let _ = enc.write_all(data);
                    let _ = enc.flush();
                }
                out
            }
        }
    }
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}

fn idat_payload() -> Vec<u8> {
    // Eine Zeile: Filtertyp 0 + ein Graustufen-Pixel.
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    let _ = enc.write_all(&[0, 0x7f]);
    enc.finish().unwrap_or_default()
}

/// Kleinste mögliche Größe, die [`png_of_size`] erzeugen kann.
pub fn png_min_size() -> usize {
    // Signatur + IHDR + IDAT + IEND + leerer Padding-Chunk (12 Byte).
    8 + 25 + (12 + idat_payload().len()) + 12 + 12
}

/// Erzeugt ein gültiges 1×1-PNG mit **genau** `size` Byte. Die Größe wird über einen privaten
/// Hilfs-Chunk (`paDd`, ancillary/private/safe-to-copy) eingestellt, den Decoder überspringen.
/// `None`, wenn `size` kleiner als [`png_min_size`] ist.
pub fn png_of_size(size: usize) -> Option<Vec<u8>> {
    let min = png_min_size();
    if size < min {
        return None;
    }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(&PNG_SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&1u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat_payload());
    chunk(&mut out, b"paDd", &vec![0u8; size - min]);
    chunk(&mut out, b"IEND", &[]);
    debug_assert_eq!(out.len(), size);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn png_has_exact_size() {
        for size in [png_min_size(), png_min_size() + 1, 1000, 100_000] {
            assert_eq!(png_of_size(size).unwrap().len(), size);
        }
        assert!(png_of_size(png_min_size() - 1).is_none());
    }

    #[test]
    fn encodings_round_trip() {
        let data = b"hello hello hello hello".repeat(20);
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(&Encoding::Gzip.encode(&data)[..])
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, data);
        out.clear();
        flate2::read::ZlibDecoder::new(&Encoding::Deflate.encode(&data)[..])
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, data);
        out.clear();
        flate2::read::DeflateDecoder::new(&Encoding::DeflateRaw.encode(&data)[..])
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, data);
        out.clear();
        brotli::Decompressor::new(&Encoding::Br.encode(&data)[..], 4096)
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, data);
        out.clear();
        assert!(
            flate2::read::GzDecoder::new(&Encoding::BrokenGzip.encode(&data)[..])
                .read_to_end(&mut out)
                .is_err()
        );
    }
}
