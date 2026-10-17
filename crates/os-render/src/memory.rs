//! Resident memory of Chromium's process tree, without shell commands.
#[cfg(target_os = "linux")]
pub fn resident_bytes(pid: u32) -> Option<u64> {
    let text = std::fs::read_to_string(
        std::path::Path::new("/proc")
            .join(pid.to_string())
            .join("status"),
    )
    .ok()?;
    text.lines().find_map(|l| {
        l.strip_prefix("VmRSS:")
            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            .map(|v| v * 1024)
    })
}
#[cfg(target_os = "macos")]
pub fn resident_bytes(pid: u32) -> Option<u64> {
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut std::ffi::c_void,
            size: i32,
        ) -> i32;
    }
    // PROC_PIDTASKINFO starts with virtual_size and resident_size (two u64).
    // The oversized aligned buffer accommodates the complete kernel structure.
    let mut info = [0u64; 32];
    // SAFETY: writable aligned buffer of the advertised size, no retained pointers.
    let count = unsafe {
        proc_pidinfo(
            pid as i32,
            4,
            0,
            info.as_mut_ptr().cast(),
            std::mem::size_of_val(&info) as i32,
        )
    };
    (count >= 16).then_some(info[1])
}
#[cfg(target_os = "windows")]
pub fn resident_bytes(pid: u32) -> Option<u64> {
    #[repr(C)]
    struct Counters {
        size: u32,
        faults: u32,
        peak: usize,
        working: usize,
        pool_peak: usize,
        pool: usize,
        nonpaged_peak: usize,
        nonpaged: usize,
        pagefile: usize,
        pagefile_peak: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn K32GetProcessMemoryInfo(
            handle: *mut std::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }
    // SAFETY: process handle is closed on every successful open; counters has the
    // documented PROCESS_MEMORY_COUNTERS layout and is valid for the call.
    unsafe {
        let handle = OpenProcess(0x0400 | 0x0010, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut counters: Counters = std::mem::zeroed();
        counters.size = std::mem::size_of::<Counters>() as u32;
        let ok = K32GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<Counters>() as u32,
        );
        CloseHandle(handle);
        (ok != 0).then_some(counters.working as u64)
    }
}
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn resident_bytes(_pid: u32) -> Option<u64> {
    None
}
