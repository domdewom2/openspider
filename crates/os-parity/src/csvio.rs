//! Toleranter CSV-Leser für Referenz- und OpenSpider-Exporte.
//!
//! Toleriert UTF-8-BOM, CRLF/LF, Komma/Semikolon/Tab als Trennzeichen (Autodetektion anhand der
//! Kopfzeile), Anführungszeichen mit eingebetteten Trennzeichen und Zeilenumbrüchen, unterschiedlich
//! lange Zeilen sowie eine führende Excel-Zeile `sep=;`. Beschreibungsspalten der Referenz werden
//! beim Einlesen verworfen (`drop`), ihre Inhalte gelangen nie in den Speicher der Auswertung
//! (Clean-Room, Abschnitt 2 Regel 1).

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::ParityError;
use crate::mapping::{Mapping, RefFormat};

/// Eingelesene Tabelle: Kopfzeile plus Datenzeilen gleicher Länge.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    /// Spaltennamen in Dateireihenfolge (ohne verworfene Spalten; doppelte Namen erhalten `" (2)"` usw.).
    pub columns: Vec<String>,
    /// Datenzeilen; jede hat genau `columns.len()` Zellen.
    pub rows: Vec<Vec<String>>,
    /// Namen der verworfenen Spalten (so geschrieben wie in der Datei).
    pub dropped: Vec<String>,
}

impl Table {
    /// Index der Spalte mit genau diesem Namen.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }
}

/// Liest eine CSV-Datei; `drop` nennt zu verwerfende Spalten (Vergleich ohne Beachtung der Schreibweise).
pub fn read_csv(path: &Path, drop: &[String]) -> Result<Table, ParityError> {
    let bytes = std::fs::read(path).map_err(|e| {
        ParityError::Invalid(format!("{} kann nicht gelesen werden: {e}", path.display()))
    })?;
    parse_csv(&bytes, drop).map_err(|e| match e {
        ParityError::Invalid(m) => ParityError::Invalid(format!("{}: {m}", path.display())),
        other => other,
    })
}

/// Wie [`read_csv`], aber aus Bytes.
pub fn parse_csv(bytes: &[u8], drop: &[String]) -> Result<Table, ParityError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    let mut text: &str = &text;
    // Excel-Hinweiszeile `sep=;` überspringen.
    if let Some(rest) = text.strip_prefix("sep=") {
        if let Some(nl) = rest.find('\n') {
            if rest[..nl].trim_end_matches('\r').chars().count() == 1 {
                text = &rest[nl + 1..];
            }
        }
    }
    let delimiter = detect_delimiter(text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());
    let mut records = reader.records();
    let header: Vec<String> = match records.next() {
        Some(r) => r
            .map_err(|e| ParityError::Invalid(format!("Kopfzeile: {e}")))?
            .iter()
            .map(|c| c.trim().to_string())
            .collect(),
        None => return Ok(Table::default()),
    };
    let width = header.len();
    let drop_lower: Vec<String> = drop.iter().map(|d| d.trim().to_lowercase()).collect();
    let keep: Vec<usize> = (0..width)
        .filter(|&i| !drop_lower.contains(&header[i].to_lowercase()))
        .collect();
    let dropped: Vec<String> = (0..width)
        .filter(|i| !keep.contains(i))
        .map(|i| header[i].clone())
        .collect();
    let mut columns: Vec<String> = Vec::with_capacity(keep.len());
    for &i in &keep {
        let mut name = header[i].clone();
        let mut n = 2;
        while columns.contains(&name) {
            name = format!("{} ({n})", header[i]);
            n += 1;
        }
        columns.push(name);
    }
    let mut rows = Vec::new();
    for (n, rec) in records.enumerate() {
        let rec = rec.map_err(|e| ParityError::Invalid(format!("Zeile {}: {e}", n + 2)))?;
        let row: Vec<String> = keep
            .iter()
            .map(|&i| rec.get(i).unwrap_or("").to_string())
            .collect();
        rows.push(row);
    }
    Ok(Table {
        columns,
        rows,
        dropped,
    })
}

/// Spaltennamen der Tabelle, in die [`parse_blocks`] das Blockformat überführt.
pub const BLOCK_COLUMNS: [&str; 4] = ["Section", "Item", "URLs", "% of Total"];

/// Liest die Referenzdatei nach dem Layout des Mappings (Tabelle oder Blockformat).
pub fn read_reference(path: &Path, mapping: &Mapping) -> Result<Table, ParityError> {
    match mapping.format {
        RefFormat::Table => read_csv(path, &mapping.drop_ref_columns),
        RefFormat::CrawlPath => {
            let mut table = read_csv(path, &mapping.drop_ref_columns)?;
            table
                .rows
                .retain(|row| !row.iter().all(|cell| cell.trim().is_empty()));
            for name in [
                "Source",
                "Destination",
                "Type",
                "Anchor",
                "Crawl Depth",
                "Status Code",
            ] {
                required(&table, name)?;
            }
            Ok(table)
        }
        RefFormat::Xml => crate::xml::read_sitemap(path),
        RefFormat::Blocks => {
            let bytes = std::fs::read(path).map_err(|e| {
                ParityError::Invalid(format!("{} kann nicht gelesen werden: {e}", path.display()))
            })?;
            parse_blocks(&bytes, &mapping.blocks.section_map).map_err(|e| match e {
                ParityError::Invalid(m) => ParityError::Invalid(format!("{}: {m}", path.display())),
                other => other,
            })
        }
    }
}

/// Liest OpenSpider-Ausgaben; Block-Mappings beziehen sich nur auf die Referenz.
pub fn read_output(path: &Path, mapping: &Mapping) -> Result<Table, ParityError> {
    if mapping.format == RefFormat::CrawlPath {
        project_crawl_path(read_csv(path, &[])?)
    } else if mapping.format == RefFormat::Xml {
        crate::xml::read_sitemap(path)
    } else {
        read_csv(path, &[])
    }
}

/// Überführt das Blockformat des Crawl-Overview-Exports der Referenz in eine Tabelle
/// `(Section, Item, URLs, % of Total)`.
///
/// Aufbau der Datei: zuerst Schlüssel/Wert-Zeilen (`Site Crawled`, Datum, Uhrzeit, …) bis zur ersten Leerzeile
/// – sie gehören nicht zu den Zählern und werden verworfen. Danach folgen durch Leerzeilen getrennte
/// Abschnitte. Ein Abschnitt beginnt mit einer Zeile, die entweder nur den Abschnittsnamen enthält oder
/// den Namen gefolgt von den Spaltenköpfen (`Summary,URLs,% of Total,Total URLs,…`); die weiteren Zeilen sind
/// Einträge `Name,URLs,% of Total[,Gesamtzahl,Beschreibung]`. Die Beschreibungsspalte wird nie eingelesen
/// (Clean-Room). Der Prozentwert bleibt als Text erhalten (Dezimalkomma und `%` normalisiert das Mapping).
/// `section_map` benennt Abschnitte der Referenz um.
pub fn parse_blocks(
    bytes: &[u8],
    section_map: &BTreeMap<String, String>,
) -> Result<Table, ParityError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = String::from_utf8_lossy(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut section: Option<String> = None;
    let mut last_section: Option<String> = None;
    let mut preamble = true;
    let mut seen_any = false;
    for (n, rec) in reader.records().enumerate() {
        let rec = rec.map_err(|e| ParityError::Invalid(format!("Zeile {}: {e}", n + 1)))?;
        let cells: Vec<String> = rec.iter().map(|c| c.trim().to_string()).collect();
        let blank = cells.iter().all(|c| c.is_empty());
        if blank {
            if seen_any {
                preamble = false;
            }
            section = None;
            continue;
        }
        seen_any = true;
        if preamble {
            continue;
        }
        if section.is_none() {
            // Zeile mit Spaltenköpfen (`Summary,URLs,…`) oder nur dem Abschnittsnamen.
            if cells.len() == 1 || cells.get(1).is_some_and(|c| c == "URLs") {
                let name = cells[0].clone();
                section = Some(section_map.get(&name).cloned().unwrap_or(name));
                last_section = section.clone();
                continue;
            }
            // Ein Eintrag ohne Kopf nach einer Leerzeile setzt den vorigen Abschnitt fort (Summary hat
            // Teilblöcke).
            match &last_section {
                Some(_) => section = last_section.clone(),
                None => {
                    return Err(ParityError::Invalid(format!(
                        "Zeile {}: Abschnittskopf erwartet, gefunden {:?}",
                        n + 1,
                        cells
                    )))
                }
            }
        }
        if cells.len() < 3 {
            return Err(ParityError::Invalid(format!(
                "Zeile {}: Eintrag mit mindestens drei Zellen erwartet",
                n + 1
            )));
        }
        rows.push(vec![
            section.clone().unwrap_or_default(),
            cells[0].clone(),
            cells[1].clone(),
            cells[2].clone(),
        ]);
    }
    if rows.is_empty() {
        return Err(ParityError::Invalid(
            "kein Abschnitt im Blockformat gefunden".into(),
        ));
    }
    Ok(Table {
        columns: BLOCK_COLUMNS.iter().map(|c| c.to_string()).collect(),
        rows,
        dropped: vec!["Total URLs".into(), "Total URLs Description".into()],
    })
}

/// Bestimmt das Trennzeichen anhand der ersten Zeile (außerhalb von Anführungszeichen):
/// häufigstes von `,` `;` Tab, bei Gleichstand das Komma.
pub fn detect_delimiter(text: &str) -> u8 {
    let (mut comma, mut semi, mut tab) = (0usize, 0usize, 0usize);
    let mut in_quotes = false;
    for c in text.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            '\n' if !in_quotes => break,
            ',' if !in_quotes => comma += 1,
            ';' if !in_quotes => semi += 1,
            '\t' if !in_quotes => tab += 1,
            _ => {}
        }
    }
    if semi > comma && semi >= tab {
        b';'
    } else if tab > comma && tab > semi {
        b'\t'
    } else {
        b','
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_comma_csv() {
        let t = parse_csv(b"Address,Status Code\nhttp://a/,200\nhttp://b/,404\n", &[]).unwrap();
        assert_eq!(t.columns, ["Address", "Status Code"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1], ["http://b/", "404"]);
    }

    #[test]
    fn bom_and_crlf_are_tolerated() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(b"Address,Status Code\r\nhttp://a/,200\r\n\r\nhttp://b/,404\r\n");
        let t = parse_csv(&data, &[]).unwrap();
        assert_eq!(t.columns[0], "Address", "BOM darf nicht im Namen stehen");
        assert_eq!(t.rows.len(), 2, "Leerzeilen werden übersprungen");
        assert_eq!(t.rows[0], ["http://a/", "200"]);
    }

    #[test]
    fn semicolon_is_detected() {
        let t = parse_csv(b"Address;Title\nhttp://a/;Hallo, Welt\n", &[]).unwrap();
        assert_eq!(t.columns, ["Address", "Title"]);
        assert_eq!(t.rows[0][1], "Hallo, Welt");
    }

    #[test]
    fn delimiters_inside_quotes_do_not_count() {
        assert_eq!(detect_delimiter("\"a;b;c\",d\n"), b',');
        assert_eq!(detect_delimiter("a\tb\tc\n"), b'\t');
        assert_eq!(detect_delimiter("a,b;c;d\n"), b';');
        assert_eq!(detect_delimiter("single\n"), b',');
    }

    #[test]
    fn quotes_with_embedded_newline_and_escaped_quote() {
        let t = parse_csv(b"A,B\n\"x\ny\",\"say \"\"hi\"\"\"\n", &[]).unwrap();
        assert_eq!(t.rows[0], ["x\ny", "say \"hi\""]);
    }

    #[test]
    fn ragged_rows_are_padded_and_extra_cells_cut() {
        let t = parse_csv(b"A,B,C\n1\n1,2,3,4\n", &[]).unwrap();
        assert_eq!(t.rows[0], ["1", "", ""]);
        assert_eq!(t.rows[1], ["1", "2", "3"]);
    }

    #[test]
    fn sep_hint_line_is_skipped() {
        let t = parse_csv(b"sep=;\nA;B\n1;2\n", &[]).unwrap();
        assert_eq!(t.columns, ["A", "B"]);
    }

    #[test]
    fn description_columns_are_dropped_on_read() {
        let t = parse_csv(
            b"Address,Description,How To Fix,Status Code\nhttp://a/,geheimer Text,noch einer,200\n",
            &["description".to_string(), "How To Fix".to_string()],
        )
        .unwrap();
        assert_eq!(t.columns, ["Address", "Status Code"]);
        assert_eq!(t.rows[0], ["http://a/", "200"]);
        assert_eq!(t.dropped, ["Description", "How To Fix"]);
        assert!(
            !t.rows.iter().flatten().any(|c| c.contains("geheim")),
            "verworfene Inhalte dürfen nicht im Speicher bleiben"
        );
    }

    #[test]
    fn duplicate_headers_get_suffix() {
        let t = parse_csv(b"A,A,A\n1,2,3\n", &[]).unwrap();
        assert_eq!(t.columns, ["A", "A (2)", "A (3)"]);
    }

    #[test]
    fn empty_input_is_empty_table() {
        assert_eq!(parse_csv(b"", &[]).unwrap(), Table::default());
        let t = parse_csv(b"A,B\n", &[]).unwrap();
        assert!(t.rows.is_empty());
        assert_eq!(t.columns.len(), 2);
    }
}

fn required(table: &Table, name: &str) -> Result<usize, ParityError> {
    table
        .index_of(name)
        .ok_or_else(|| ParityError::Invalid(format!("Crawl Path: Spalte {name} fehlt")))
}

/// OUT-023: validate complete, ordered target blocks before projecting adjacent nodes.
pub fn project_crawl_path(table: Table) -> Result<Table, ParityError> {
    let indices = [
        "Target Address",
        "Step",
        "Address",
        "Status Code",
        "Link Type",
        "Anchor",
        "Crawl Depth",
    ]
    .map(|name| required(&table, name))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let mut result = Table {
        columns: [
            "Target Address",
            "Source",
            "Destination",
            "Type",
            "Anchor",
            "Crawl Depth",
            "Status Code",
        ]
        .map(String::from)
        .to_vec(),
        ..Default::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut offset = 0;
    while offset < table.rows.len() {
        let target = &table.rows[offset][indices[0]];
        if target.is_empty() || !seen.insert(target.clone()) {
            return Err(ParityError::Invalid(
                "Crawl Path: vermischte Zielblöcke".into(),
            ));
        }
        let end = (offset..table.rows.len())
            .find(|&i| table.rows[i][indices[0]] != *target)
            .unwrap_or(table.rows.len());
        let block = &table.rows[offset..end];
        let mut addresses = std::collections::BTreeSet::new();
        for (step, row) in block.iter().enumerate() {
            if row[indices[1]].parse::<usize>().ok() != Some(step)
                || !addresses.insert(&row[indices[2]])
                || row[indices[2]].is_empty()
            {
                return Err(ParityError::Invalid(
                    "Crawl Path: fehlender Knoten, falsche Steps oder Zyklus".into(),
                ));
            }
        }
        let root = block.last().expect("nonempty block");
        if block[0][indices[2]] != *target
            || root[indices[6]] != "0"
            || !root[indices[4]].is_empty()
            || !root[indices[5]].is_empty()
        {
            return Err(ParityError::Invalid(
                "Crawl Path: falsches Ziel oder unvollständiger Startknoten".into(),
            ));
        }
        for pair in block.windows(2) {
            let destination = &pair[0];
            let source = &pair[1];
            if destination[indices[4]].is_empty() {
                return Err(ParityError::Invalid(
                    "Crawl Path: Verbindungsart fehlt".into(),
                ));
            }
            result.rows.push(vec![
                target.clone(),
                source[indices[2]].clone(),
                destination[indices[2]].clone(),
                destination[indices[4]].clone(),
                destination[indices[5]].clone(),
                source[indices[6]].clone(),
                destination[indices[3]].clone(),
            ]);
        }
        offset = end;
    }
    Ok(result)
}

/// The reference has edges without target labels. Require one complete chain ending at the selection;
/// never discard an unrecognised or disconnected row. Blank separators are removed at read time only.
pub fn label_crawl_path(table: &mut Table, target_path: &str) -> Result<(), ParityError> {
    let source = required(table, "Source")?;
    let destination = required(table, "Destination")?;
    let depth = required(table, "Crawl Depth")?;
    let targets: Vec<_> = table
        .rows
        .iter()
        .filter(|row| row[destination].ends_with(target_path))
        .collect();
    if targets.len() != 1 {
        return Err(ParityError::Invalid(
            "Crawl Path: falsche oder mehrdeutige Referenz-Zielauswahl".into(),
        ));
    }
    let target = targets[0][destination].clone();
    let base = target
        .strip_suffix(target_path)
        .ok_or_else(|| ParityError::Invalid("Crawl Path: Zielpfad fehlt".into()))?;
    let mut cursor = format!("{base}/");
    let mut used = std::collections::BTreeSet::new();
    while cursor != target {
        let next: Vec<_> = table
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row[source] == cursor)
            .collect();
        if next.len() != 1 {
            return Err(ParityError::Invalid(
                "Crawl Path: fehlende oder mehrdeutige Referenzverbindung".into(),
            ));
        }
        let (index, row) = next[0];
        if row[depth].parse::<usize>().ok() != Some(used.len()) || !used.insert(index) {
            return Err(ParityError::Invalid(
                "Crawl Path: Referenztiefe oder Zyklus".into(),
            ));
        }
        cursor = row[destination].clone();
    }
    if used.len() != table.rows.len() {
        return Err(ParityError::Invalid(
            "Crawl Path: unverbundene Referenzzeilen".into(),
        ));
    }
    table.columns.push("Target Address".into());
    for row in &mut table.rows {
        row.push(target.clone());
    }
    Ok(())
}

#[cfg(test)]
mod crawl_path_tests {
    use super::*;
    fn nodes() -> Table {
        parse_csv(b"Target Address,Step,Address,Status Code,Link Type,Anchor,Crawl Depth\nhttp://a/end,0,http://a/end,200,Redirect,,2\nhttp://a/end,1,http://a/mid,301,Hyperlink,Go,1\nhttp://a/end,2,http://a/,200,,,0\n", &[]).unwrap()
    }
    // spec: OUT-023
    #[test]
    fn projects_redirect_destination_status_and_source_depth() {
        let table = project_crawl_path(nodes()).unwrap();
        assert_eq!(
            table.rows,
            [
                [
                    "http://a/end",
                    "http://a/mid",
                    "http://a/end",
                    "Redirect",
                    "",
                    "1",
                    "200"
                ],
                [
                    "http://a/end",
                    "http://a/",
                    "http://a/mid",
                    "Hyperlink",
                    "Go",
                    "0",
                    "301"
                ],
            ]
        );
        let mut two = nodes();
        let mut second = nodes();
        for row in &mut second.rows {
            for cell in row {
                *cell = cell.replace("http://a", "http://b");
            }
        }
        two.rows.extend(second.rows);
        assert_eq!(project_crawl_path(two).unwrap().rows.len(), 4);
    }
    // spec: OUT-023
    #[test]
    fn rejects_missing_reversed_mixed_or_wrong_target_nodes() {
        let mut missing = nodes();
        missing.rows.remove(1);
        assert!(project_crawl_path(missing).is_err());
        let mut reversed = nodes();
        reversed.rows.reverse();
        assert!(project_crawl_path(reversed).is_err());
        let mut wrong = nodes();
        wrong.rows[0][2] = "http://a/wrong".into();
        assert!(project_crawl_path(wrong).is_err());
        let mut mixed = nodes();
        mixed.rows[1][0] = "http://b/end".into();
        assert!(project_crawl_path(mixed).is_err());
    }
    // spec: OUT-023
    #[test]
    fn validates_reference_chain_and_preserves_values_for_comparison() {
        let mut edges = project_crawl_path(nodes()).unwrap();
        edges.columns.remove(0);
        for row in &mut edges.rows {
            row.remove(0);
        }
        label_crawl_path(&mut edges.clone(), "/end").unwrap();
        assert!(label_crawl_path(&mut edges.clone(), "/wrong").is_err());
        let mut missing = edges.clone();
        missing.rows.pop();
        assert!(label_crawl_path(&mut missing, "/end").is_err());
        let mut wrong_depth = edges.clone();
        wrong_depth.rows[1][4] = "4".into();
        assert!(label_crawl_path(&mut wrong_depth, "/end").is_err());
        let mut anchor = edges.clone();
        anchor.rows[1][3] = "Wrong".into();
        label_crawl_path(&mut anchor, "/end").unwrap();
        assert_eq!(
            anchor.rows[1][3], "Wrong",
            "Anchor bleibt uneingeschränkt im Zellvergleich"
        );
    }
}

#[cfg(test)]
mod crawl_path_comparison_tests {
    use super::*;
    // spec: OUT-023
    #[test]
    fn wrong_anchor_depth_or_missing_connection_cannot_pass_the_mapping() {
        let reference = parse_csv(b"Target Address,Source,Destination,Type,Anchor,Crawl Depth,Status Code\nhttp://a/end,http://a/,http://a/mid,Hyperlink,Go,0,301\nhttp://a/end,http://a/mid,http://a/end,Redirect,,1,200\n", &[]).unwrap();
        let mapping = Mapping::parse(include_str!(
            "../../../tests/parity/mappings/crawl_path.toml"
        ))
        .unwrap();
        let bases = crate::normalize::BaseUrls::from_base("http://a");
        let compare = |output: &Table| {
            crate::diff::compare(&crate::diff::CompareInput {
                export: "crawl_path",
                site: None,
                reference: &reference,
                os: output,
                mapping: &mapping,
                ref_bases: &bases,
                os_bases: &bases,
                deviations: &[],
            })
            .unwrap()
        };
        let ok = compare(&reference);
        assert_eq!(ok.cells_equal, ok.cells_compared);
        for column in ["Anchor", "Crawl Depth", "Status Code"] {
            let mut wrong = reference.clone();
            wrong.rows[0][required(&reference, column).unwrap()] = "wrong".into();
            let report = compare(&wrong);
            assert!(report.cells_equal < report.cells_compared, "{column}");
        }
        let mut missing = reference.clone();
        missing.rows.pop();
        assert!(!compare(&missing).missing_in_os.is_empty());
    }
}
