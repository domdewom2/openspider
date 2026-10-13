//! Cargo-Ziele (Bibliothek, Binärdateien, Integrationstests) und die von ihnen erreichbaren Quelldateien.
//!
//! Ein Testergebnis nennt nur das Test-Binär (`unittests src/lib.rs`, `tests/x.rs`) und den Testpfad.
//! Damit daraus (Datei, Funktion) wird, folgt dieses Modul den `mod name;`-Deklarationen ab der
//! Wurzeldatei jedes Ziels und liefert für jede erreichbare Datei ihren Modulpfad.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::coverage::results::{BinaryRef, NextestKind};
use crate::coverage::scan::{FileScan, TestFn};
use crate::error::ParityError;

/// Art eines Ziels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    /// Bibliothek.
    Lib,
    /// Binärdatei.
    Bin,
    /// Integrationstest.
    Test,
}

/// Ein Cargo-Ziel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Paketname (mit `-`).
    pub package: String,
    /// Name des Ziels (`-` → `_`).
    pub name: String,
    /// Rohname laut Manifest.
    pub raw_name: String,
    /// Art.
    pub kind: TargetKind,
    /// Wurzeldatei relativ zur Repository-Wurzel.
    pub root: String,
    /// Paketverzeichnis relativ zur Repository-Wurzel.
    pub crate_dir: String,
}

/// Fügt `rel` an `base` an und löst `.`/`..` auf (nur `/`-getrennte relative Pfade).
pub fn join_norm(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(p, _)| p)
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, f)| f)
}

/// Findet alle Pakete unter `crates/` (und ihre Ziele).
pub fn discover_targets(root: &Path) -> Result<Vec<Target>, ParityError> {
    let mut manifests = Vec::new();
    collect_manifests(root, "crates", &mut manifests);
    manifests.sort();
    let mut targets = Vec::new();
    for dir in manifests {
        let text = std::fs::read_to_string(root.join(&dir).join("Cargo.toml"))?;
        let Ok(doc) = text.parse::<toml::Table>() else {
            continue;
        };
        let Some(package) = doc
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        else {
            continue;
        };
        let exists = |rel: &str| root.join(join_norm(&dir, rel)).is_file();
        let mut add = |kind: TargetKind, raw: &str, path: String| {
            if !targets
                .iter()
                .any(|t: &Target| t.crate_dir == dir && t.kind == kind && t.raw_name == raw)
            {
                targets.push(Target {
                    package: package.to_string(),
                    name: raw.replace('-', "_"),
                    raw_name: raw.to_string(),
                    kind,
                    root: join_norm(&dir, &path),
                    crate_dir: dir.clone(),
                });
            }
        };
        // Explizit deklarierte Ziele.
        let lib = doc.get("lib").and_then(|l| l.as_table());
        let lib_path = lib
            .and_then(|l| l.get("path"))
            .and_then(|p| p.as_str())
            .unwrap_or("src/lib.rs")
            .to_string();
        if lib.is_some() || exists("src/lib.rs") {
            let name = lib
                .and_then(|l| l.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(package);
            add(TargetKind::Lib, name, lib_path);
        }
        for (key, kind, default_dir) in [
            ("bin", TargetKind::Bin, "src/bin"),
            ("test", TargetKind::Test, "tests"),
        ] {
            if let Some(arr) = doc.get(key).and_then(|b| b.as_array()) {
                for b in arr.iter().filter_map(|b| b.as_table()) {
                    let Some(name) = b.get("name").and_then(|n| n.as_str()) else {
                        continue;
                    };
                    let path = b
                        .get("path")
                        .and_then(|p| p.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            if kind == TargetKind::Bin && name == package && exists("src/main.rs") {
                                "src/main.rs".to_string()
                            } else {
                                format!("{default_dir}/{name}.rs")
                            }
                        });
                    add(kind, name, path);
                }
            }
        }
        // Automatisch erkannte Ziele.
        if exists("src/main.rs") {
            add(TargetKind::Bin, package, "src/main.rs".to_string());
        }
        for (kind, sub) in [(TargetKind::Bin, "src/bin"), (TargetKind::Test, "tests")] {
            let Ok(rd) = std::fs::read_dir(root.join(&dir).join(sub)) else {
                continue;
            };
            let mut found: Vec<(String, String)> = Vec::new();
            for e in rd.filter_map(|e| e.ok()) {
                let name = e.file_name().to_string_lossy().into_owned();
                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    if root
                        .join(&dir)
                        .join(sub)
                        .join(&name)
                        .join("main.rs")
                        .is_file()
                    {
                        found.push((name.clone(), format!("{sub}/{name}/main.rs")));
                    }
                } else if let Some(stem) = name.strip_suffix(".rs") {
                    found.push((stem.to_string(), format!("{sub}/{name}")));
                }
            }
            found.sort();
            for (name, path) in found {
                add(kind, &name, path);
            }
        }
    }
    Ok(targets)
}

fn collect_manifests(root: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(root.join(rel)) else {
        return;
    };
    for e in rd.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().into_owned();
        if matches!(name.as_str(), "target" | "node_modules" | ".git") {
            continue;
        }
        let child = format!("{rel}/{name}");
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if root.join(&child).join("Cargo.toml").is_file() {
                out.push(child.clone());
            }
            collect_manifests(root, &child, out);
        }
    }
}

/// Verzeichnis, in dem `mod name;` einer Datei gesucht wird.
fn module_dir(file: &str, is_root: bool) -> String {
    let name = file_name(file);
    if is_root || name == "mod.rs" || name == "lib.rs" || name == "main.rs" {
        parent(file).to_string()
    } else {
        let stem = name.strip_suffix(".rs").unwrap_or(name);
        join_norm(parent(file), stem)
    }
}

/// Erreichbare Dateien eines Ziels mit ihrem Modulpfad (Wurzel = leer).
pub fn reachable_files(
    target: &Target,
    scans: &BTreeMap<String, FileScan>,
) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack: Vec<(String, Vec<String>, bool)> = vec![(target.root.clone(), Vec::new(), true)];
    while let Some((file, modpath, is_root)) = stack.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let Some(scan) = scans.get(&file) else {
            continue;
        };
        let dir = module_dir(&file, is_root);
        for m in &scan.mods {
            let mut sub = modpath.clone();
            sub.extend(m.inline.iter().cloned());
            sub.push(m.name.clone());
            let mut base = dir.clone();
            for inl in &m.inline {
                base = join_norm(&base, inl);
            }
            let candidates: Vec<String> = match &m.path_attr {
                Some(p) => vec![join_norm(parent(&file), p)],
                None => vec![
                    join_norm(&base, &format!("{}.rs", m.name)),
                    join_norm(&base, &format!("{}/mod.rs", m.name)),
                ],
            };
            if let Some(c) = candidates.into_iter().find(|c| scans.contains_key(c)) {
                stack.push((c, sub, false));
            }
        }
        out.push((file, modpath));
    }
    out.sort();
    out
}

/// Index: Zielindex → vollständiger Testpfad (`mod::pfad::fn`) → Testindizes.
pub struct TargetIndex {
    /// Alle Ziele.
    pub targets: Vec<Target>,
    /// Pro Ziel: Testpfad → Indizes in die globale Testliste.
    pub by_path: Vec<HashMap<String, Vec<usize>>>,
}

/// Baut den Index für die Testliste `tests` (Reihenfolge = globale Testindizes).
pub fn build_index(
    targets: Vec<Target>,
    scans: &BTreeMap<String, FileScan>,
    tests: &[TestFn],
) -> TargetIndex {
    let mut by_file: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, t) in tests.iter().enumerate() {
        by_file.entry(t.file.as_str()).or_default().push(i);
    }
    let by_path = targets
        .iter()
        .map(|t| {
            let mut map: HashMap<String, Vec<usize>> = HashMap::new();
            for (file, modpath) in reachable_files(t, scans) {
                for &ti in by_file.get(file.as_str()).into_iter().flatten() {
                    let test = &tests[ti];
                    let mut parts: Vec<&str> = modpath.iter().map(String::as_str).collect();
                    parts.extend(test.module.iter().map(String::as_str));
                    parts.push(&test.name);
                    map.entry(parts.join("::")).or_default().push(ti);
                }
            }
            map
        })
        .collect();
    TargetIndex { targets, by_path }
}

impl TargetIndex {
    /// Indizes der Ziele, in denen das Binär eines Ergebnisses stecken kann.
    pub fn candidates(&self, bin: &BinaryRef) -> Vec<usize> {
        let norm = |s: &str| s.replace('-', "_");
        let all = self.targets.iter().enumerate();
        match bin {
            BinaryRef::Cargo { unit, rel, name } => {
                let by_name: Vec<usize> = all
                    .filter(|(_, t)| {
                        t.name == *name
                            && if *unit {
                                matches!(t.kind, TargetKind::Lib | TargetKind::Bin)
                            } else {
                                t.kind == TargetKind::Test
                            }
                    })
                    .map(|(i, _)| i)
                    .collect();
                let exact: Vec<usize> = by_name
                    .iter()
                    .copied()
                    .filter(|&i| self.targets[i].root == join_norm(&self.targets[i].crate_dir, rel))
                    .collect();
                if exact.is_empty() {
                    by_name
                } else {
                    exact
                }
            }
            BinaryRef::Nextest { package, kind } => all
                .filter(|(_, t)| {
                    t.package == *package
                        && match kind {
                            NextestKind::Lib => t.kind == TargetKind::Lib,
                            NextestKind::Bin(n) => t.kind == TargetKind::Bin && t.name == norm(n),
                            NextestKind::Test(n) => t.kind == TargetKind::Test && t.name == norm(n),
                        }
                })
                .map(|(i, _)| i)
                .collect(),
        }
    }
}

impl TargetIndex {
    /// Grenzt mehrdeutige Ziele (gleicher Name und Wurzelpfad in verschiedenen Paketen) anhand der Testnamen
    /// ein, die im selben Ausgabeblock stehen: erst Mengengleichheit, dann Teilmenge.
    pub fn narrow(&self, candidates: Vec<usize>, block_paths: &[&str]) -> Vec<usize> {
        if candidates.len() < 2 {
            return candidates;
        }
        let paths: HashSet<&str> = block_paths.iter().copied().collect();
        let equal: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&c| {
                let own = &self.by_path[c];
                own.len() == paths.len() && paths.iter().all(|p| own.contains_key(*p))
            })
            .collect();
        if !equal.is_empty() {
            return equal;
        }
        let subset: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&c| paths.iter().all(|p| self.by_path[c].contains_key(*p)))
            .collect();
        if subset.is_empty() {
            candidates
        } else {
            subset
        }
    }
}

/// Zuordnung eines Testpfads zu getaggten Tests.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Genau eine Testfunktion.
    One(usize),
    /// Mehrere mögliche Testfunktionen (Warnung, keine Gutschrift).
    Ambiguous(Vec<usize>),
    /// Test nicht in den gescannten Quellen (z. B. untagged macro).
    None,
}

impl TargetIndex {
    /// Ordnet `path` (Testpfad aus der Ausgabe) den Quellen zu.
    pub fn resolve(&self, candidates: &[usize], path: &str) -> Resolved {
        let mut hits: Vec<usize> = Vec::new();
        for &c in candidates {
            if let Some(v) = self.by_path[c].get(path) {
                for &t in v {
                    if !hits.contains(&t) {
                        hits.push(t);
                    }
                }
            }
        }
        if hits.is_empty() {
            // Rückfall: nur der Funktionsname (z. B. Tests aus Makros in anderen Modulen).
            let name = path.rsplit("::").next().unwrap_or(path);
            for &c in candidates {
                for (k, v) in &self.by_path[c] {
                    if k.rsplit("::").next() == Some(name) {
                        for &t in v {
                            if !hits.contains(&t) {
                                hits.push(t);
                            }
                        }
                    }
                }
            }
        }
        match hits.len() {
            0 => Resolved::None,
            1 => Resolved::One(hits[0]),
            _ => {
                hits.sort_unstable();
                Resolved::Ambiguous(hits)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_norm_resolves_dots() {
        assert_eq!(join_norm("crates/a", "src/lib.rs"), "crates/a/src/lib.rs");
        assert_eq!(join_norm("crates/a", "../../tests/x.rs"), "tests/x.rs");
        assert_eq!(join_norm("", "./a//b"), "a/b");
    }

    #[test]
    fn module_dir_rules() {
        assert_eq!(module_dir("crates/a/src/lib.rs", true), "crates/a/src");
        assert_eq!(module_dir("crates/a/src/x.rs", false), "crates/a/src/x");
        assert_eq!(module_dir("crates/a/src/x/mod.rs", false), "crates/a/src/x");
        assert_eq!(module_dir("crates/a/tests/t.rs", true), "crates/a/tests");
    }
}
