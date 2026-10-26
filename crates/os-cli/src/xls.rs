//! Small disk-backed BIFF8/CFB target for legacy CLI output (OUT-195).
//! Strings are explicit SST cells, so spreadsheet formulas remain inert text.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

const END: u32 = 0xffff_fffe;
const FREE: u32 = 0xffff_ffff;
fn record(w: &mut File, kind: u16, data: &[u8]) -> io::Result<()> {
    w.write_all(&kind.to_le_bytes())?;
    w.write_all(&(data.len() as u16).to_le_bytes())?;
    w.write_all(data)
}
fn bof(w: &mut File, kind: u16) -> io::Result<()> {
    let mut data = Vec::new();
    for n in [0x0600u16, kind, 0x0dbb, 0x07cc, 0x0041, 0, 0x0006, 0] {
        data.extend(n.to_le_bytes());
    }
    record(w, 0x0809, &data)
}
fn reader(path: &Path) -> io::Result<(Vec<String>, impl Iterator<Item = io::Result<Vec<String>>>)> {
    let (header, rows) = os_export::csv::read_table(path).map_err(err)?;
    Ok((header, Box::new(rows.map(|r| r.map_err(err)))))
}
fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
struct Strings<'a> {
    file: &'a mut File,
    data: Vec<u8>,
    first: bool,
}
impl Strings<'_> {
    fn flush(&mut self) -> io::Result<()> {
        record(
            self.file,
            if self.first { 0x00fc } else { 0x003c },
            &self.data,
        )?;
        self.first = false;
        self.data.clear();
        Ok(())
    }
    fn string(&mut self, text: &str) -> io::Result<bool> {
        let mut units: Vec<u16> = text.encode_utf16().take(32767).collect();
        if units.last().is_some_and(|u| (0xd800..=0xdbff).contains(u)) {
            units.pop();
        }
        let truncated = text.encode_utf16().count() > units.len();
        if self.data.len() + 3 > 8224 {
            self.flush()?;
        }
        self.data.extend((units.len() as u16).to_le_bytes());
        self.data.push(1);
        for unit in units {
            if self.data.len() + 2 > 8224 {
                self.flush()?;
                self.data.push(1);
            }
            self.data.extend(unit.to_le_bytes());
        }
        Ok(truncated)
    }
}
/// Converts one table without collecting its rows; limits are the BIFF8 worksheet limits.
pub fn convert(csv: &Path, destination: &Path) -> io::Result<Vec<String>> {
    convert_with(csv, destination, &os_model::CancelToken::new())
}

pub fn convert_with(
    csv: &Path,
    destination: &Path,
    token: &os_model::CancelToken,
) -> io::Result<Vec<String>> {
    token.checkpoint().map_err(err)?;
    let (header, mut input) = reader(csv)?;
    let rows = input.try_fold(0u64, |n, r| {
        token.checkpoint().map_err(err)?;
        r.map(|_| n + 1)
    })?;
    let width = header.len().min(256);
    let sheets = rows.div_ceil(65535).max(1) as usize;
    let strings = (rows + 1)
        .checked_mul(width as u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| err("XLS string limit exceeded"))?;
    let staging =
        crate::staging::Staging::new(destination.parent().unwrap_or_else(|| Path::new(".")))?;
    let pending = staging.path.join("workbook.xls");
    let stream_path = staging.path.join("stream.biff");
    let result = (|| {
        let mut file = File::create(&stream_path)?;
        bof(&mut file, 5)?;
        record(&mut file, 0x0042, &1200u16.to_le_bytes())?;
        // One default cell XF, explicitly no formulas or external references.
        record(&mut file, 0x00e0, &[0; 20])?;
        let mut offsets = Vec::new();
        for i in 0..sheets {
            let name = if i == 0 {
                "Export".to_string()
            } else {
                format!("Export ({})", i + 1)
            };
            offsets.push(file.stream_position()? + 4);
            let mut data = vec![0; 6];
            data.extend([name.len() as u8, 0]);
            data.extend(name.bytes());
            record(&mut file, 0x0085, &data)?;
        }
        let mut sst = Strings {
            file: &mut file,
            data: Vec::new(),
            first: true,
        };
        sst.data.extend(strings.to_le_bytes());
        sst.data.extend(strings.to_le_bytes());
        let mut truncated = false;
        for text in header.iter().take(width) {
            truncated |= sst.string(text.trim_start_matches('\u{feff}'))?;
        }
        for row in reader(csv)?.1 {
            token.checkpoint().map_err(err)?;
            let row = row.map_err(err)?;
            for col in 0..width {
                truncated |= sst.string(row.get(col).map(String::as_str).unwrap_or_default())?;
            }
        }
        sst.flush()?;
        record(&mut file, 0x000a, &[])?;
        let (_, mut records) = reader(csv)?;
        let mut index = width as u32;
        for (sheet, offset) in offsets.into_iter().enumerate() {
            let start = file.stream_position()?;
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(&(u32::try_from(start).map_err(err)?).to_le_bytes())?;
            file.seek(SeekFrom::Start(start))?;
            bof(&mut file, 0x0010)?;
            let count = rows.saturating_sub(sheet as u64 * 65535).min(65535) as u32;
            let mut dimensions = Vec::new();
            dimensions.extend(0u32.to_le_bytes());
            dimensions.extend((count + 1).to_le_bytes());
            dimensions.extend(0u16.to_le_bytes());
            dimensions.extend((width as u16).to_le_bytes());
            dimensions.extend([0; 2]);
            record(&mut file, 0x0200, &dimensions)?;
            for row in 0..=count {
                token.checkpoint().map_err(err)?;
                if row > 0 {
                    records
                        .next()
                        .ok_or_else(|| err("missing CSV row"))?
                        .map_err(err)?;
                }
                for col in 0..width {
                    let mut data = Vec::new();
                    data.extend((row as u16).to_le_bytes());
                    data.extend((col as u16).to_le_bytes());
                    data.extend(0u16.to_le_bytes());
                    data.extend(if row == 0 {
                        (col as u32).to_le_bytes()
                    } else {
                        let v = index;
                        index += 1;
                        v.to_le_bytes()
                    });
                    record(&mut file, 0x00fd, &data)?;
                }
            }
            record(&mut file, 0x000a, &[])?;
        }
        drop(file);
        compound(&stream_path, &pending)?;
        #[cfg(windows)]
        if destination.exists() {
            std::fs::remove_file(destination)?;
        }
        std::fs::rename(&pending, destination)?;
        let mut warnings = Vec::new();
        if header.len() > 256 {
            warnings.push("XLS: columns beyond 256 were omitted".into());
        }
        if sheets > 1 {
            warnings.push("XLS: rows continued on additional worksheets".into());
        }
        if truncated {
            warnings.push("XLS: text truncated to 32767 UTF-16 units".into());
        }
        Ok(warnings)
    })();
    let _ = std::fs::remove_file(stream_path);
    result
}
fn compound(stream: &Path, destination: &Path) -> io::Result<()> {
    let size = std::fs::metadata(stream)?.len().max(4096);
    let sectors = size.div_ceil(512) as u32;
    let (mut fat, mut difat) = (1u32, 0u32);
    loop {
        let next = (sectors + 1 + fat + difat).div_ceil(128);
        let next_difat = next.saturating_sub(109).div_ceil(127);
        if (fat, difat) == (next, next_difat) {
            break;
        }
        (fat, difat) = (next, next_difat);
    }
    let mut header = [0u8; 512];
    header[..8].copy_from_slice(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
    header[24..34].copy_from_slice(&[0x3e, 0, 3, 0, 0xfe, 0xff, 9, 0, 6, 0]);
    for (pos, n) in [
        (44, fat),
        (48, sectors),
        (56, 4096),
        (60, END),
        (68, if difat > 0 { sectors + 1 + fat } else { END }),
        (72, difat),
    ] {
        header[pos..pos + 4].copy_from_slice(&n.to_le_bytes());
    }
    for i in 0..109 {
        header[76 + i * 4..80 + i * 4].copy_from_slice(&if i < fat as usize {
            (sectors + 1 + i as u32).to_le_bytes()
        } else {
            FREE.to_le_bytes()
        });
    }
    let mut out = File::create(destination)?;
    out.write_all(&header)?;
    let mut input = File::open(stream)?;
    let copied = io::copy(&mut input, &mut out)?;
    io::copy(
        &mut io::repeat(0).take(u64::from(sectors) * 512 - copied),
        &mut out,
    )?;
    let mut dir = [0u8; 512];
    for (base, name, kind, child, start, length) in [
        (0, "Root Entry", 5, 1, END, 0u64),
        (128, "Workbook", 2, FREE, 0, size),
    ] {
        let encoded: Vec<_> = name.encode_utf16().chain([0]).collect();
        for (i, u) in encoded.iter().enumerate() {
            dir[base + i * 2..base + i * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        dir[base + 64..base + 66].copy_from_slice(&((encoded.len() * 2) as u16).to_le_bytes());
        dir[base + 66] = kind;
        dir[base + 67] = 1;
        for (pos, n) in [(68, FREE), (72, FREE), (76, child), (116, start)] {
            dir[base + pos..base + pos + 4].copy_from_slice(&n.to_le_bytes());
        }
        dir[base + 120..base + 128].copy_from_slice(&length.to_le_bytes());
    }
    out.write_all(&dir)?;
    for i in 0..fat * 128 {
        let n = if i < sectors {
            if i + 1 < sectors {
                i + 1
            } else {
                END
            }
        } else if i == sectors {
            END
        } else if i < sectors + 1 + fat {
            0xffff_fffd
        } else if i < sectors + 1 + fat + difat {
            0xffff_fffc
        } else {
            FREE
        };
        out.write_all(&n.to_le_bytes())?;
    }
    for d in 0..difat {
        for i in 0..127 {
            let idx = 109 + d * 127 + i;
            out.write_all(&if idx < fat {
                (sectors + 1 + idx).to_le_bytes()
            } else {
                FREE.to_le_bytes()
            })?;
        }
        out.write_all(&if d + 1 < difat {
            (sectors + 1 + fat + d + 1).to_le_bytes()
        } else {
            END.to_le_bytes()
        })?;
    }
    out.flush()
}
