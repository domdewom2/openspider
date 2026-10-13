//! Spitzen-RSS (PERF-001, PERF-003) plattformübergreifend.
//!
//! Der Crawl läuft in einem **Kindprozess**; der Elternprozess misst dessen Speicherspitze:
//!
//! | Plattform | Quelle |
//! |---|---|
//! | macOS | `wait4`: `rusage.ru_maxrss` des beendeten Kindes (Bytes) |
//! | Windows | `GetProcessMemoryInfo`: `PeakWorkingSetSize` des Kindes (während des Laufs und nach dem Ende abgefragt) |
//! | übrige Unix-Systeme | `VmHWM` aus `/proc/<pid>/status` (zyklisch abgefragt; `ru_maxrss` ist nach `vfork`/`exec` unzuverlässig) |
//!
//! [`peak_rss_self`] liefert dieselbe Größe für den eigenen Prozess (`getrusage`, `VmHWM`,
//! `GetProcessMemoryInfo`).

use std::io;
use std::process::Child;

/// Ende eines Kindprozesses samt Speicherspitze.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildExit {
    /// Exit-Code 0.
    pub success: bool,
    /// Exit-Code, falls das Kind regulär endete (nicht durch ein Signal).
    pub code: Option<i32>,
    /// Spitzen-RSS in Bytes (`None`, wenn die Plattform keinen Wert lieferte).
    pub peak_rss_bytes: Option<u64>,
}

/// Wartet auf das Ende des Kindes und liefert dessen Spitzen-RSS.
///
/// Auf macOS reapt der Aufruf das Kind selbst (`wait4`); danach darf `Child::wait`/`try_wait` nicht mehr
/// aufgerufen werden. Auf den anderen Plattformen bleibt das `Child` normal benutzbar.
pub fn wait_with_peak_rss(child: &mut Child) -> io::Result<ChildExit> {
    imp::wait_with_peak_rss(child)
}

/// Spitzen-RSS des eigenen Prozesses in Bytes.
pub fn peak_rss_self() -> Option<u64> {
    imp::peak_rss_self()
}

/// Wandelt `KB`-Angaben aus `/proc/<pid>/status` (`VmHWM:   1234 kB`) in Bytes.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
fn parse_vm_hwm(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kib: u64 = line
        .trim_start_matches("VmHWM:")
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    kib.checked_mul(1024)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::ChildExit;
    use std::io;
    use std::process::Child;

    pub fn wait_with_peak_rss(child: &mut Child) -> io::Result<ChildExit> {
        let pid = libc::pid_t::try_from(child.id())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid zu groß"))?;
        let mut status: libc::c_int = 0;
        // SAFETY: `rusage` ist ein reiner C-Datensatz, den Nullen gültig initialisieren.
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        loop {
            // SAFETY: `status` und `usage` sind gültige, beschreibbare Zeiger für die Dauer des Aufrufs.
            let r = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
            if r == pid {
                break;
            }
            let err = io::Error::last_os_error();
            if r == -1 && err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        let code = libc::WIFEXITED(status).then(|| libc::WEXITSTATUS(status));
        Ok(ChildExit {
            success: code == Some(0),
            code,
            // macOS meldet `ru_maxrss` in Bytes
            peak_rss_bytes: u64::try_from(usage.ru_maxrss).ok().filter(|&b| b > 0),
        })
    }

    pub fn peak_rss_self() -> Option<u64> {
        // SAFETY: wie oben; `getrusage` schreibt nur in `usage`.
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
        (r == 0)
            .then(|| u64::try_from(usage.ru_maxrss).ok())
            .flatten()
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use super::{parse_vm_hwm, ChildExit};
    use std::io;
    use std::process::Child;
    use std::time::Duration;

    fn vm_hwm_of(pid: &str) -> Option<u64> {
        parse_vm_hwm(&std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?)
    }

    pub fn wait_with_peak_rss(child: &mut Child) -> io::Result<ChildExit> {
        let pid = child.id().to_string();
        let mut peak = 0u64;
        loop {
            if let Some(v) = vm_hwm_of(&pid) {
                peak = peak.max(v);
            }
            if let Some(status) = child.try_wait()? {
                return Ok(ChildExit {
                    success: status.success(),
                    code: status.code(),
                    peak_rss_bytes: (peak > 0).then_some(peak),
                });
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn peak_rss_self() -> Option<u64> {
        vm_hwm_of("self")
    }
}

#[cfg(windows)]
mod imp {
    use super::ChildExit;
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    fn peak_working_set(handle: HANDLE) -> Option<u64> {
        // SAFETY: Nullen sind für diesen reinen Datensatz gültig; `cb` wird wie verlangt gesetzt, und `handle`
        // bleibt für die Dauer des Aufrufs gültig (gehört dem `Child` bzw. ist der Pseudo-Handle des Prozesses).
        unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let ok = GetProcessMemoryInfo(handle, &mut counters, counters.cb);
            (ok != 0).then(|| counters.PeakWorkingSetSize as u64)
        }
    }

    pub fn wait_with_peak_rss(child: &mut Child) -> io::Result<ChildExit> {
        let handle = child.as_raw_handle() as HANDLE;
        let mut peak = 0u64;
        loop {
            if let Some(v) = peak_working_set(handle) {
                peak = peak.max(v);
            }
            if let Some(status) = child.try_wait()? {
                // Das Handle bleibt bis zum Drop des `Child` offen: letzte Abfrage nach dem Ende.
                if let Some(v) = peak_working_set(handle) {
                    peak = peak.max(v);
                }
                return Ok(ChildExit {
                    success: status.success(),
                    code: status.code(),
                    peak_rss_bytes: (peak > 0).then_some(peak),
                });
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn peak_rss_self() -> Option<u64> {
        // SAFETY: `GetCurrentProcess` liefert den Pseudo-Handle des eigenen Prozesses und hat keine Vorbedingung.
        peak_working_set(unsafe { GetCurrentProcess() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vm_hwm_from_proc_status() {
        let status = "Name:\tx\nVmPeak:\t  900 kB\nVmHWM:\t    1234 kB\nVmRSS:\t 1000 kB\n";
        assert_eq!(parse_vm_hwm(status), Some(1234 * 1024));
        assert_eq!(parse_vm_hwm("Name:\tx\n"), None);
        assert_eq!(parse_vm_hwm("VmHWM:\tabc kB\n"), None);
    }

    // spec: PERF-001
    #[test]
    fn own_peak_rss_is_plausible_and_grows_with_allocations() {
        let before = peak_rss_self().expect("RSS des eigenen Prozesses");
        assert!(before > 1024 * 1024, "{before}");
        // 64 MiB anfassen, damit sie wirklich resident werden
        let mut big = vec![0u8; 64 * 1024 * 1024];
        for i in (0..big.len()).step_by(4096) {
            big[i] = 1;
        }
        std::hint::black_box(&big);
        let after = peak_rss_self().unwrap();
        assert!(after >= before, "{before} -> {after}");
    }
}
