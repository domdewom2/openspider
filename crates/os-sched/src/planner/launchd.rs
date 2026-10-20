//! macOS: LaunchAgent `de.openspider.task.<id>.plist` mit `StartCalendarInterval` (OUT-166).
//!
//! Der Agent läuft im Benutzerkontext, wenn der Benutzer angemeldet ist. launchd holt Termine, die der
//! Rechner im Ruhezustand verpasst hat, höchstens einmal nach dem Aufwachen nach; eine Begrenzung der
//! Laufzeit gibt es nicht. Die Optionen „im Akkubetrieb ausführen“ und „Nachholen“ sind unter macOS
//! dadurch ohnehin erfüllt und erscheinen nicht in der plist.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use time::Month;

use super::{
    write_file, xml_escape, Backend, CommandRunner, OsPlanner, PlannerError, PlannerFile,
    PlannerSpec, Preview,
};
use crate::tasks::schedule::{parse_instant, parse_local, parse_time};
use crate::tasks::Schedule;

/// Präfix der Agent-Bezeichnungen.
pub const LABEL_PREFIX: &str = "de.openspider.task.";

/// Bezeichnung des Agents einer Aufgabe.
pub fn label(task_id: &str) -> String {
    format!("{LABEL_PREFIX}{task_id}")
}

fn entry(fields: &[(&str, u32)], indent: &str) -> String {
    let mut out = format!("{indent}<dict>\n");
    for (key, value) in fields {
        out.push_str(&format!(
            "{indent}\t<key>{key}</key>\n{indent}\t<integer>{value}</integer>\n"
        ));
    }
    out.push_str(&format!("{indent}</dict>\n"));
    out
}

/// Die Einträge für `StartCalendarInterval`.
fn calendar_entries(schedule: &Schedule) -> Result<Vec<Vec<(&'static str, u32)>>, PlannerError> {
    let invalid = |what: &str| PlannerError::Invalid(format!("ungültiger Zeitplan: {what}"));
    let hm = |time: &str| {
        parse_time(time)
            .map(|t| (u32::from(t.hour()), u32::from(t.minute())))
            .ok_or_else(|| invalid("Uhrzeit"))
    };
    Ok(match schedule {
        Schedule::Daily { time, .. } => {
            let (h, m) = hm(time)?;
            vec![vec![("Hour", h), ("Minute", m)]]
        }
        Schedule::Weekly { days, time, .. } => {
            let (h, m) = hm(time)?;
            if days.is_empty() {
                return Err(invalid("Wochentage"));
            }
            let mut days = days.clone();
            days.sort_unstable();
            days.dedup();
            days.iter()
                .map(|d| {
                    // launchd: 0 und 7 sind Sonntag, 1 = Montag.
                    vec![("Weekday", u32::from(*d % 7)), ("Hour", h), ("Minute", m)]
                })
                .collect()
        }
        Schedule::Monthly { day, time, .. } => {
            let (h, m) = hm(time)?;
            if *day <= 28 {
                vec![vec![("Day", u32::from(*day)), ("Hour", h), ("Minute", m)]]
            } else {
                // Kurze Monate laufen am letzten Tag; launchd kennt keinen „letzten Tag“, daher je Monat
                // ein Eintrag (Februar: 28., auch im Schaltjahr).
                (1u8..=12)
                    .map(|month| {
                        let length = Month::try_from(month).map_or(28, |mo| mo.length(2001));
                        vec![
                            ("Month", u32::from(month)),
                            ("Day", u32::from((*day).min(length))),
                            ("Hour", h),
                            ("Minute", m),
                        ]
                    })
                    .collect()
            }
        }
        Schedule::Once { at } => {
            let t = parse_instant(at).ok_or_else(|| invalid("Zeitpunkt"))?;
            vec![vec![
                ("Month", u32::from(u8::from(t.month()))),
                ("Day", u32::from(t.day())),
                ("Hour", u32::from(t.hour())),
                ("Minute", u32::from(t.minute())),
            ]]
        }
        Schedule::Hourly { start } => {
            let t = parse_local(start).ok_or_else(|| invalid("Startzeitpunkt"))?;
            vec![vec![("Minute", u32::from(t.minute()))]]
        }
        Schedule::EveryDays { every: 1, start } => {
            let t = parse_local(start).ok_or_else(|| invalid("Startzeitpunkt"))?;
            vec![vec![
                ("Hour", u32::from(t.hour())),
                ("Minute", u32::from(t.minute())),
            ]]
        }
        Schedule::EveryDays { every, .. } => {
            return Err(PlannerError::Unsupported(format!(
                "launchd kennt keinen Abstand von {every} Tagen"
            )))
        }
    })
}

/// Erzeugt die plist einer Aufgabe.
pub fn plist(spec: &PlannerSpec) -> Result<String, PlannerError> {
    let entries = calendar_entries(&spec.schedule)?;
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n");
    out.push_str("<plist version=\"1.0\">\n<dict>\n");
    out.push_str(&format!(
        "\t<key>Label</key>\n\t<string>{}</string>\n",
        xml_escape(&label(&spec.task_id))
    ));
    out.push_str("\t<key>ProgramArguments</key>\n\t<array>\n");
    for arg in [
        spec.binary.to_string_lossy().into_owned(),
        "--run-task".to_string(),
        spec.task_id.clone(),
    ] {
        out.push_str(&format!("\t\t<string>{}</string>\n", xml_escape(&arg)));
    }
    out.push_str("\t</array>\n");
    out.push_str("\t<key>StartCalendarInterval</key>\n");
    if let [single] = entries.as_slice() {
        out.push_str(&entry(single, "\t"));
    } else {
        out.push_str("\t<array>\n");
        for e in &entries {
            out.push_str(&entry(e, "\t\t"));
        }
        out.push_str("\t</array>\n");
    }
    if let Some(home) = &spec.home {
        out.push_str("\t<key>EnvironmentVariables</key>\n\t<dict>\n");
        out.push_str(&format!(
            "\t\t<key>OPENSPIDER_HOME</key>\n\t\t<string>{}</string>\n",
            xml_escape(&home.to_string_lossy())
        ));
        out.push_str("\t</dict>\n");
    }
    out.push_str("</dict>\n</plist>\n");
    Ok(out)
}

/// LaunchAgent-Backend.
pub struct Launchd {
    dir: PathBuf,
    runner: Arc<dyn CommandRunner>,
}

impl Launchd {
    /// Backend mit Agent-Ordner (`~/Library/LaunchAgents`) und Programmaufruf.
    pub fn new(dir: PathBuf, runner: Arc<dyn CommandRunner>) -> Self {
        Launchd { dir, runner }
    }

    fn path(&self, task_id: &str) -> PathBuf {
        self.dir.join(format!("{}.plist", label(task_id)))
    }

    fn command(&self, spec: &PlannerSpec) -> Vec<String> {
        vec![
            "launchctl".into(),
            "load".into(),
            "-w".into(),
            self.path(&spec.task_id).to_string_lossy().into_owned(),
        ]
    }

    fn launchctl(&self, args: &[&str]) -> Result<super::CommandOutput, PlannerError> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        self.runner
            .run("launchctl", &args)
            .map_err(|source| PlannerError::Io {
                path: PathBuf::from("launchctl"),
                source,
            })
    }
}

impl OsPlanner for Launchd {
    fn backend(&self) -> Backend {
        Backend::Launchd
    }

    fn preview(&self, spec: &PlannerSpec) -> Result<Preview, PlannerError> {
        Ok(Preview {
            backend: Backend::Launchd,
            files: vec![PlannerFile {
                path: self.path(&spec.task_id),
                content: plist(spec)?,
            }],
            command: self.command(spec),
        })
    }

    fn install(&self, spec: &PlannerSpec) -> Result<Preview, PlannerError> {
        let preview = self.preview(spec)?;
        let file = &preview.files[0];
        // Ein vorhandener Eintrag wird zuerst entladen, damit launchd die neue Fassung liest.
        if file.path.exists() {
            let _ = self.launchctl(&["unload", &file.path.to_string_lossy()]);
        }
        write_file(&file.path, file.content.as_bytes())?;
        let out = self.launchctl(&["load", "-w", &file.path.to_string_lossy()])?;
        if out.status != 0 {
            return Err(PlannerError::Command {
                program: "launchctl".into(),
                status: out.status,
                stderr: out.stderr.trim().to_string(),
            });
        }
        Ok(preview)
    }

    fn uninstall(&self, task_id: &str) -> Result<(), PlannerError> {
        let path = self.path(task_id);
        if !path.exists() {
            return Ok(());
        }
        let _ = self.launchctl(&["unload", &path.to_string_lossy()]);
        std::fs::remove_file(&path).map_err(|source| PlannerError::Io { path, source })
    }

    fn is_registered(&self, task_id: &str) -> bool {
        self.path(task_id).is_file()
    }

    fn registered_ids(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|n| {
                n.strip_prefix(LABEL_PREFIX)
                    .and_then(|r| r.strip_suffix(".plist"))
                    .map(str::to_string)
            })
            .collect();
        ids.sort();
        ids
    }

    fn registered_binary(&self, task_id: &str) -> Option<PathBuf> {
        let text = std::fs::read_to_string(self.path(task_id)).ok()?;
        let after = text.split("<key>ProgramArguments</key>").nth(1)?;
        let first = after.split("<string>").nth(1)?.split("</string>").next()?;
        Some(PathBuf::from(
            first
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .replace("&amp;", "&"),
        ))
    }
}

/// Pfad der plist einer Aufgabe in `dir` (für Tests und Anzeigen).
pub fn plist_path(dir: &Path, task_id: &str) -> PathBuf {
    dir.join(format!("{}.plist", label(task_id)))
}
