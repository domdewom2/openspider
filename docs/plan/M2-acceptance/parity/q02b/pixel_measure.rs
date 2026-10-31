//! Temporary native measurement: numeric title widths through os-parity's CSV reader only.
fn main() {
    let root = std::path::Path::new("tests/golden/parity");
    let mut result = Vec::new();
    for item in std::fs::read_dir(root).unwrap() {
        let dir = item.unwrap().path();
        let manifest = dir.join("manifest.toml");
        if !manifest.is_file() {
            continue;
        }
        let manifest = os_parity::manifest::Manifest::load(&manifest).unwrap();
        for export in manifest.export {
            if export.ref_file != "internal_all.csv" {
                continue;
            }
            let mapping = os_parity::mapping::Mapping::load(
                &std::path::Path::new("tests/parity/mappings")
                    .join(format!("{}.toml", export.mapping)),
            )
            .unwrap();
            let table =
                os_parity::csvio::read_reference(&dir.join("ref").join(export.ref_file), &mapping)
                    .unwrap();
            let Some(title) = table.index_of("Title 1") else {
                continue;
            };
            let Some(width) = table.index_of("Title 1 Pixel Width") else {
                continue;
            };
            for row in table.rows {
                if let Ok(px) = row[width].parse::<u32>() {
                    result.push(serde_json::json!({"site":dir.file_name().unwrap().to_str().unwrap(),"title":row[title],"px":px}));
                }
            }
        }
    }
    println!("{}", serde_json::to_string(&result).unwrap());
}
