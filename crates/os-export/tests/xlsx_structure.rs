//! Struktur der XLSX-Datei (OUT-003): fixierte Kopfzeile, Autofilter, keine Formeln, kein Hyperlink,
//! Datumsformat. Die Prüfung liest das ZIP selbst (Zentralverzeichnis + Inflate nach RFC 1951), weil im
//! Workspace kein ZIP-Leser für Tests eingebunden ist.

use std::collections::HashMap;
use std::path::Path;

use os_analyze::ColumnKind;
use os_export::output::open_writer;
use os_export::{Format, WriterOptions};
use os_model::Value;

// ---------------------------------------------------------------------------------------------------------
// Minimaler ZIP-Leser
// ---------------------------------------------------------------------------------------------------------

fn u16le(b: &[u8], o: usize) -> usize {
    usize::from(u16::from_le_bytes([b[o], b[o + 1]]))
}
fn u32le(b: &[u8], o: usize) -> usize {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize
}

fn read_zip(path: &Path) -> HashMap<String, Vec<u8>> {
    let b = std::fs::read(path).unwrap();
    let eocd = (0..=b.len() - 22)
        .rev()
        .find(|&i| b[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .expect("EOCD");
    let count = u16le(&b, eocd + 10);
    let mut off = u32le(&b, eocd + 16);
    let mut out = HashMap::new();
    for _ in 0..count {
        assert_eq!(
            b[off..off + 4],
            [0x50, 0x4b, 0x01, 0x02],
            "Zentralverzeichnis"
        );
        let method = u16le(&b, off + 10);
        let csize = u32le(&b, off + 20);
        let nlen = u16le(&b, off + 28);
        let elen = u16le(&b, off + 30);
        let clen = u16le(&b, off + 32);
        let local = u32le(&b, off + 42);
        let name = String::from_utf8(b[off + 46..off + 46 + nlen].to_vec()).unwrap();
        let l_nlen = u16le(&b, local + 26);
        let l_elen = u16le(&b, local + 28);
        let data = &b[local + 30 + l_nlen + l_elen..][..csize];
        let content = match method {
            0 => data.to_vec(),
            8 => inflate(data),
            m => panic!("Kompressionsmethode {m}"),
        };
        out.insert(name, content);
        off += 46 + nlen + elen + clen;
    }
    out
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    bit: u32,
    cnt: u32,
}

impl Bits<'_> {
    fn need(&mut self, n: u32) -> u32 {
        while self.cnt < n {
            let byte = *self.data.get(self.pos).expect("Datenende");
            self.pos += 1;
            self.bit |= u32::from(byte) << self.cnt;
            self.cnt += 8;
        }
        let v = self.bit & ((1u32 << n) - 1);
        self.bit >>= n;
        self.cnt -= n;
        v
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

fn huff(lengths: &[u8]) -> Huff {
    let mut count = [0u16; 16];
    for &l in lengths {
        count[usize::from(l)] += 1;
    }
    let mut offs = [0u16; 16];
    for i in 1..15 {
        offs[i + 1] = offs[i] + count[i];
    }
    let mut symbol = vec![0u16; lengths.len()];
    for (s, &l) in lengths.iter().enumerate() {
        if l != 0 {
            symbol[usize::from(offs[usize::from(l)])] = s as u16;
            offs[usize::from(l)] += 1;
        }
    }
    Huff { count, symbol }
}

fn decode(b: &mut Bits, h: &Huff) -> usize {
    let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
    for len in 1..16 {
        code |= b.need(1) as i32;
        let count = i32::from(h.count[len]);
        if code - count < first {
            return usize::from(h.symbol[(index + (code - first)) as usize]);
        }
        index += count;
        first += count;
        first <<= 1;
        code <<= 1;
    }
    panic!("ungültiger Huffman-Code");
}

const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate(data: &[u8]) -> Vec<u8> {
    let mut b = Bits {
        data,
        pos: 0,
        bit: 0,
        cnt: 0,
    };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.need(1);
        match b.need(2) {
            0 => {
                b.bit = 0;
                b.cnt = 0;
                let len = u16le(data, b.pos);
                b.pos += 4;
                out.extend_from_slice(&data[b.pos..b.pos + len]);
                b.pos += len;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [0u8; 288];
                    for (i, v) in l.iter_mut().enumerate() {
                        *v = match i {
                            0..=143 => 8,
                            144..=255 => 9,
                            256..=279 => 7,
                            _ => 8,
                        };
                    }
                    (huff(&l), huff(&[5u8; 30]))
                } else {
                    let nlen = b.need(5) as usize + 257;
                    let ndist = b.need(5) as usize + 1;
                    let ncode = b.need(4) as usize + 4;
                    const ORDER: [usize; 19] = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut cl = [0u8; 19];
                    for &o in ORDER.iter().take(ncode) {
                        cl[o] = b.need(3) as u8;
                    }
                    let clh = huff(&cl);
                    let mut lengths = vec![0u8; nlen + ndist];
                    let mut i = 0;
                    while i < nlen + ndist {
                        let sym = decode(&mut b, &clh);
                        if sym < 16 {
                            lengths[i] = sym as u8;
                            i += 1;
                        } else {
                            let (prev, rep) = match sym {
                                16 => (lengths[i - 1], 3 + b.need(2)),
                                17 => (0, 3 + b.need(3)),
                                _ => (0, 11 + b.need(7)),
                            };
                            for _ in 0..rep {
                                lengths[i] = prev;
                                i += 1;
                            }
                        }
                    }
                    (huff(&lengths[..nlen]), huff(&lengths[nlen..]))
                };
                loop {
                    let sym = decode(&mut b, &lit);
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let s = sym - 257;
                        let len = usize::from(LBASE[s]) + b.need(u32::from(LEXT[s])) as usize;
                        let ds = decode(&mut b, &dist);
                        let d = usize::from(DBASE[ds]) + b.need(u32::from(DEXT[ds])) as usize;
                        for _ in 0..len {
                            out.push(out[out.len() - d]);
                        }
                    }
                }
            }
            _ => panic!("ungültiger Block"),
        }
        if last == 1 {
            return out;
        }
    }
}

fn text_of(files: &HashMap<String, Vec<u8>>, name: &str) -> String {
    String::from_utf8(
        files
            .get(name)
            .unwrap_or_else(|| panic!("{name} fehlt"))
            .clone(),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------------------

fn sample(path: &Path, rows: i64, opts: &WriterOptions) {
    let mut w = open_writer(path, Format::Xlsx, "Internal", opts).unwrap();
    w.header(&[
        "Address".into(),
        "Status Code".into(),
        "Crawl Timestamp".into(),
        "Title".into(),
    ])
    .unwrap();
    for i in 0..rows {
        w.row(
            &[
                ColumnKind::Url,
                ColumnKind::Int,
                ColumnKind::Timestamp,
                ColumnKind::Text,
            ],
            &[
                Value::Text(format!("https://example.com/{i}")),
                Value::Int(200),
                Value::Text("2026-10-13T09:35:00+02:00".into()),
                Value::Text("=SUM(A1)".into()),
            ],
        )
        .unwrap();
    }
    w.finish().unwrap();
}

// spec: OUT-003
#[test]
fn header_row_is_frozen_and_has_autofilter_over_all_columns() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("s.xlsx");
    sample(&path, 5, &WriterOptions::default());
    let files = read_zip(&path);
    let sheet = text_of(&files, "xl/worksheets/sheet1.xml");
    assert!(sheet.contains("<pane "), "{sheet}");
    assert!(sheet.contains(r#"ySplit="1""#), "{sheet}");
    assert!(sheet.contains(r#"state="frozen""#), "{sheet}");
    assert!(sheet.contains(r#"<autoFilter ref="A1:D6"/>"#), "{sheet}");
    // Excel verlangt für den Autofilter den definierten Namen
    let wb = text_of(&files, "xl/workbook.xml");
    assert!(wb.contains("_xlnm._FilterDatabase"), "{wb}");
}

// spec: OUT-003
#[test]
fn autofilter_and_freeze_apply_to_every_overflow_sheet() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("o.xlsx");
    let opts = WriterOptions {
        max_rows_per_sheet: 2,
        ..WriterOptions::default()
    };
    sample(&path, 5, &opts);
    let files = read_zip(&path);
    let s1 = text_of(&files, "xl/worksheets/sheet1.xml");
    let s3 = text_of(&files, "xl/worksheets/sheet3.xml");
    assert!(s1.contains(r#"<autoFilter ref="A1:D3"/>"#), "{s1}");
    assert!(s3.contains(r#"<autoFilter ref="A1:D2"/>"#), "{s3}");
    assert!(s3.contains(r#"state="frozen""#), "{s3}");
}

// spec: OUT-003
#[test]
fn no_formula_hyperlink_or_link_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("f.xlsx");
    sample(&path, 3, &WriterOptions::default());
    let files = read_zip(&path);
    let sheet = text_of(&files, "xl/worksheets/sheet1.xml");
    assert!(
        !sheet.contains("<f>") && !sheet.contains("<f "),
        "keine Formelzellen"
    );
    assert!(
        !sheet.contains("<hyperlink"),
        "URLs sind Text, nie Hyperlink"
    );
    let strings = files
        .iter()
        .filter(|(k, _)| {
            k.as_str() == "xl/sharedStrings.xml" || k.starts_with("xl/worksheets/sheet")
        })
        .map(|(_, v)| String::from_utf8_lossy(v).into_owned())
        .collect::<String>();
    assert!(strings.contains("=SUM(A1)"));
}

// spec: OUT-003
#[test]
fn datetime_cells_use_the_documented_number_format() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("d.xlsx");
    sample(&path, 1, &WriterOptions::default());
    let files = read_zip(&path);
    let styles = text_of(&files, "xl/styles.xml");
    assert!(
        styles.contains(r#"formatCode="yyyy-mm-dd hh:mm:ss""#),
        "{styles}"
    );
}
