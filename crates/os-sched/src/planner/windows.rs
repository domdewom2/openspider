//! Windows: Aufgabe in der Aufgabenplanung unter `\OpenSpider\<id>` (OUT-166), beschrieben als Task-XML.
//!
//! Standard „Nur ausführen, wenn Benutzer angemeldet ist“ (`InteractiveToken`); Windows-Kennwörter speichert
//! OpenSpider nicht. Die Laufzeit ist unbegrenzt (`ExecutionTimeLimit` `PT0S`), der Akkubetrieb erlaubt,
//! ein verpasster Termin wird nachgeholt (`StartWhenAvailable`). Die Überlappung regelt der Lauf selbst
//! (OUT-167), der Planer startet immer parallel.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    write_file, xml_escape, Backend, CommandRunner, OsPlanner, PlannerError, PlannerFile,
    PlannerSpec, Preview,
};
use crate::tasks::schedule::{parse_date, parse_instant, parse_local, parse_time};
use crate::tasks::Schedule;

/// Ordner der Aufgaben in der Aufgabenplanung.
pub const FOLDER: &str = "\\OpenSpider\\";

/// Name der Aufgabe in der Aufgabenplanung.
pub fn task_name(task_id: &str) -> String {
    format!("{FOLDER}{task_id}")
}

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn trigger(start: &str, body: &str) -> String {
    format!(
        "    <CalendarTrigger>\n      <StartBoundary>{start}</StartBoundary>\n      <Enabled>true</Enabled>\n{body}    </CalendarTrigger>\n"
    )
}

fn months(list: &[usize]) -> String {
    let names: String = list.iter().map(|m| format!("<{}/>", MONTHS[*m])).collect();
    format!("        <Months>{names}</Months>\n")
}

/// Die Auslöser (`<Triggers>`-Inhalt) für einen Zeitplan.
fn triggers(schedule: &Schedule) -> Result<String, PlannerError> {
    let invalid = |what: &str| PlannerError::Invalid(format!("ungültiger Zeitplan: {what}"));
    let boundary = |from: &Option<String>, time: &str| -> Result<String, PlannerError> {
        let t = parse_time(time).ok_or_else(|| invalid("Uhrzeit"))?;
        let date = match from {
            Some(f) => parse_date(f).ok_or_else(|| invalid("Datum"))?,
            None => time::Date::from_calendar_date(2000, time::Month::January, 1)
                .map_err(|_| invalid("Datum"))?,
        };
        Ok(format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:00",
            date.year(),
            u8::from(date.month()),
            date.day(),
            t.hour(),
            t.minute()
        ))
    };
    Ok(match schedule {
        Schedule::Once { at } => {
            let t = parse_instant(at).ok_or_else(|| invalid("Zeitpunkt"))?;
            format!(
                "    <TimeTrigger>\n      <StartBoundary>{:04}-{:02}-{:02}T{:02}:{:02}:00</StartBoundary>\n      <Enabled>true</Enabled>\n    </TimeTrigger>\n",
                t.year(),
                u8::from(t.month()),
                t.day(),
                t.hour(),
                t.minute()
            )
        }
        Schedule::Daily { time, from } => trigger(
            &boundary(from, time)?,
            "      <ScheduleByDay>\n        <DaysInterval>1</DaysInterval>\n      </ScheduleByDay>\n",
        ),
        Schedule::EveryDays { every, start } => {
            let t = parse_local(start).ok_or_else(|| invalid("Startzeitpunkt"))?;
            trigger(
                &format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:00",
                    t.year(),
                    u8::from(t.month()),
                    t.day(),
                    t.hour(),
                    t.minute()
                ),
                &format!(
                    "      <ScheduleByDay>\n        <DaysInterval>{every}</DaysInterval>\n      </ScheduleByDay>\n"
                ),
            )
        }
        Schedule::Hourly { start } => {
            let t = parse_local(start).ok_or_else(|| invalid("Startzeitpunkt"))?;
            trigger(
                &format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:00",
                    t.year(),
                    u8::from(t.month()),
                    t.day(),
                    t.hour(),
                    t.minute()
                ),
                "      <Repetition>\n        <Interval>PT1H</Interval>\n        <Duration>P1D</Duration>\n      </Repetition>\n      <ScheduleByDay>\n        <DaysInterval>1</DaysInterval>\n      </ScheduleByDay>\n",
            )
        }
        Schedule::Weekly { days, time, from } => {
            if days.is_empty() || days.iter().any(|d| !(1..=7).contains(d)) {
                return Err(invalid("Wochentage"));
            }
            let mut days = days.clone();
            days.sort_unstable();
            days.dedup();
            let names: String = days
                .iter()
                .map(|d| format!("<{}/>", DAYS[usize::from(*d) - 1]))
                .collect();
            trigger(
                &boundary(from, time)?,
                &format!(
                    "      <ScheduleByWeek>\n        <DaysOfWeek>{names}</DaysOfWeek>\n        <WeeksInterval>1</WeeksInterval>\n      </ScheduleByWeek>\n"
                ),
            )
        }
        Schedule::Monthly { day, time, from } => {
            if !(1..=31).contains(day) {
                return Err(invalid("Tag des Monats"));
            }
            let start = boundary(from, time)?;
            let by_month = |days: String, list: &[usize]| {
                trigger(
                    &start,
                    &format!(
                        "      <ScheduleByMonth>\n        <DaysOfMonth>{days}</DaysOfMonth>\n{}      </ScheduleByMonth>\n",
                        months(list)
                    ),
                )
            };
            if *day <= 28 {
                by_month(format!("<Day>{day}</Day>"), &(0..12).collect::<Vec<_>>())
            } else {
                // Monate, die den Tag haben, laufen am Tag; die übrigen am letzten Tag des Monats.
                let has: Vec<usize> = (0..12)
                    .filter(|m| {
                        let length = time::Month::try_from(*m as u8 + 1).map_or(28, |mo| mo.length(2001));
                        length >= *day
                    })
                    .collect();
                let lacks: Vec<usize> = (0..12).filter(|m| !has.contains(m)).collect();
                let mut out = String::new();
                if !has.is_empty() {
                    out.push_str(&by_month(format!("<Day>{day}</Day>"), &has));
                }
                if !lacks.is_empty() {
                    out.push_str(&by_month("<Day>Last</Day>".to_string(), &lacks));
                }
                out
            }
        }
    })
}

/// Erzeugt das Task-XML einer Aufgabe.
pub fn task_xml(spec: &PlannerSpec) -> Result<String, PlannerError> {
    let triggers = triggers(&spec.schedule)?;
    let battery = if spec.on_battery { "false" } else { "true" };
    let catch_up = if spec.run_if_missed { "true" } else { "false" };
    let wake = if spec.wake { "true" } else { "false" };
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n");
    out.push_str(
        "<Task version=\"1.4\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n",
    );
    out.push_str("  <RegistrationInfo>\n    <Source>OpenSpider</Source>\n");
    out.push_str(&format!(
        "    <Description>OpenSpider: {}</Description>\n    <URI>{}</URI>\n  </RegistrationInfo>\n",
        xml_escape(&spec.task_name),
        xml_escape(&task_name(&spec.task_id))
    ));
    out.push_str(&format!("  <Triggers>\n{triggers}  </Triggers>\n"));
    out.push_str("  <Principals>\n    <Principal id=\"Author\">\n      <LogonType>InteractiveToken</LogonType>\n      <RunLevel>LeastPrivilege</RunLevel>\n    </Principal>\n  </Principals>\n");
    out.push_str("  <Settings>\n");
    out.push_str("    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>\n");
    out.push_str(&format!(
        "    <DisallowStartIfOnBatteries>{battery}</DisallowStartIfOnBatteries>\n    <StopIfGoingOnBatteries>{battery}</StopIfGoingOnBatteries>\n"
    ));
    out.push_str("    <AllowHardTerminate>true</AllowHardTerminate>\n");
    out.push_str(&format!(
        "    <StartWhenAvailable>{catch_up}</StartWhenAvailable>\n"
    ));
    out.push_str("    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>\n");
    out.push_str("    <IdleSettings>\n      <StopOnIdleEnd>false</StopOnIdleEnd>\n      <RestartOnIdle>false</RestartOnIdle>\n    </IdleSettings>\n");
    out.push_str("    <AllowStartOnDemand>true</AllowStartOnDemand>\n    <Enabled>true</Enabled>\n    <Hidden>false</Hidden>\n    <RunOnlyIfIdle>false</RunOnlyIfIdle>\n");
    out.push_str(&format!("    <WakeToRun>{wake}</WakeToRun>\n"));
    out.push_str("    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>\n    <Priority>7</Priority>\n  </Settings>\n");
    out.push_str("  <Actions Context=\"Author\">\n    <Exec>\n");
    out.push_str(&format!(
        "      <Command>{}</Command>\n      <Arguments>--run-task {}</Arguments>\n",
        xml_escape(&spec.binary.to_string_lossy()),
        xml_escape(&spec.task_id)
    ));
    out.push_str("    </Exec>\n  </Actions>\n</Task>\n");
    Ok(out)
}

/// Backend der Windows-Aufgabenplanung.
pub struct TaskScheduler {
    dir: PathBuf,
    runner: Arc<dyn CommandRunner>,
}

impl TaskScheduler {
    /// `dir` nimmt die XML-Dateien auf, die `schtasks` einliest.
    pub fn new(dir: PathBuf, runner: Arc<dyn CommandRunner>) -> Self {
        TaskScheduler { dir, runner }
    }

    fn path(&self, task_id: &str) -> PathBuf {
        self.dir.join(format!("{task_id}.xml"))
    }

    fn run(&self, args: Vec<String>) -> Result<super::CommandOutput, PlannerError> {
        self.runner
            .run("schtasks", &args)
            .map_err(|source| PlannerError::Io {
                path: PathBuf::from("schtasks"),
                source,
            })
    }
}

fn utf16le(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

impl OsPlanner for TaskScheduler {
    fn backend(&self) -> Backend {
        Backend::TaskScheduler
    }

    fn preview(&self, spec: &PlannerSpec) -> Result<Preview, PlannerError> {
        let path = self.path(&spec.task_id);
        Ok(Preview {
            backend: Backend::TaskScheduler,
            command: vec![
                "schtasks".into(),
                "/Create".into(),
                "/TN".into(),
                task_name(&spec.task_id),
                "/XML".into(),
                path.to_string_lossy().into_owned(),
                "/F".into(),
            ],
            files: vec![PlannerFile {
                path,
                content: task_xml(spec)?,
            }],
        })
    }

    fn install(&self, spec: &PlannerSpec) -> Result<Preview, PlannerError> {
        let preview = self.preview(spec)?;
        // schtasks liest XML mit der im Kopf genannten Kodierung; UTF-16 mit Byte-Order-Mark ist die
        // verlässlichste Form.
        write_file(&preview.files[0].path, &utf16le(&preview.files[0].content))?;
        let out = self.run(preview.command[1..].to_vec())?;
        if out.status != 0 {
            return Err(PlannerError::Command {
                program: "schtasks".into(),
                status: out.status,
                stderr: out.stderr.trim().to_string(),
            });
        }
        Ok(preview)
    }

    fn uninstall(&self, task_id: &str) -> Result<(), PlannerError> {
        if self.is_registered(task_id) {
            let out = self.run(vec![
                "/Delete".into(),
                "/TN".into(),
                task_name(task_id),
                "/F".into(),
            ])?;
            if out.status != 0 {
                return Err(PlannerError::Command {
                    program: "schtasks".into(),
                    status: out.status,
                    stderr: out.stderr.trim().to_string(),
                });
            }
        }
        let _ = std::fs::remove_file(self.path(task_id));
        Ok(())
    }

    fn is_registered(&self, task_id: &str) -> bool {
        self.path(task_id).is_file()
            && self
                .run(vec!["/Query".into(), "/TN".into(), task_name(task_id)])
                .is_ok_and(|o| o.status == 0)
    }

    fn registered_ids(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|n| n.strip_suffix(".xml").map(str::to_string))
            .filter(|id| self.is_registered(id))
            .collect();
        ids.sort();
        ids
    }

    fn registered_binary(&self, task_id: &str) -> Option<PathBuf> {
        let bytes = std::fs::read(self.path(task_id)).ok()?;
        let text = if bytes.starts_with(&[0xFF, 0xFE]) {
            let units: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        let command = text.split("<Command>").nth(1)?.split("</Command>").next()?;
        Some(PathBuf::from(
            command
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .replace("&amp;", "&"),
        ))
    }
}
