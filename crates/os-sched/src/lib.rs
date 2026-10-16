//! `os-sched` – Aufgabenplanung für App und CLI (OUT-160 … OUT-170, OUT-280; Klärung K2-07 entschieden).
//!
//! Gerüst von M2-F01 (`docs/plan/M2.md` 3.1/3.2/3.6): Manifest, diese Datei und je ein Stub-Modul. Die Fachlogik
//! kommt mit den Paketen M2-T01 (`tasks`, `history`, `planner`) und M2-T02 (`mail`, Besitz `src/mail/**`).
//!
//! Abhängigkeitsrichtung: `os-model`, `os-store`, `os-fetch` (nur Geheimnisse) und `lettre` ← `os-sched` ←
//! `os-app`, `os-cli`. Das Crate kennt weder `os-app` noch Tauri.
//!
//! | Modul | Inhalt | Besitzer |
//! |---|---|---|
//! | [`tasks`] | Aufgabendefinition, Validierung, Ablage (`tasks/*.toml`) | M2-T01 |
//! | [`history`] | Laufhistorie je Aufgabe | M2-T01 |
//! | [`planner`] | OS-Planer-Backends (launchd, Windows-Aufgabenplanung) mit Dry-Run | M2-T01 |
//! | [`mail`] | E-Mail-Benachrichtigung (Konten, Abschlussmail) | M2-T02 |

pub mod history;
pub mod mail;
pub mod planner;
pub mod tasks;
