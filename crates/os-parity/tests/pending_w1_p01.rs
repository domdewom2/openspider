//! Vollständiger Manifestvertrag gegen die noch von M2-D08 zu registrierenden Links-Exporte.

use std::path::Path;

// spec: OUT-033
// spec: OUT-034
// spec: OUT-035
// spec: OUT-036
// spec: OUT-037
// spec: OUT-038
// spec: OUT-039
// spec: OUT-040
// spec: OUT-041
// spec: OUT-068
#[test]
#[ignore = "wartet auf M2-D08"]
fn m2_links_bulk_manifest_resolves_against_the_catalog() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/parity/parity-m2-links/manifest.toml");
    let manifest = os_parity::manifest::Manifest::load(&path).unwrap();
    let mut checked = 0;
    for export in manifest.export {
        if let os_parity::manifest::OsExport::Bulk(name) =
            os_parity::manifest::OsExport::parse(&export.os).unwrap()
        {
            os_export::catalog::resolve(os_export::catalog::ExportKind::Bulk, &name)
                .unwrap_or_else(|e| panic!("{}: {e}", export.ref_file));
            checked += 1;
        }
    }
    assert_eq!(checked, 10);
}
