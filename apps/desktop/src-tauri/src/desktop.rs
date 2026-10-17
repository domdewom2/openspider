//! Plattformanbindung der Service-Hooks; Dateiaktionen verwenden native APIs ohne Shell.
use os_app::{
    dto::{CmdError, ErrorCode},
    runtime::hooks::{DesktopActions, WindowHost},
    App,
};
use std::{path::PathBuf, sync::Arc};
use tauri::{AppHandle, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;

struct Host<R: Runtime>(AppHandle<R>, crate::window::Quitting, std::thread::ThreadId);
fn failure(e: impl std::fmt::Display) -> CmdError {
    CmdError::new(ErrorCode::Io, "err-desktop-action").with_detail(e.to_string())
}
impl<R: Runtime> WindowHost for Host<R> {
    fn open_viz(&self, snapshot_id: &str) -> Result<(), CmdError> {
        // Kodierung über URL-Query verhindert eine Interpretation der Kennung als Pfad/Fragment.
        let mut url = tauri::Url::parse("https://localhost/").map_err(failure)?;
        url.query_pairs_mut().append_pair("snapshotId", snapshot_id);
        let path = format!("index.html?{}", url.query().unwrap_or_default());
        let window = WebviewWindowBuilder::new(
            &self.0,
            format!(
                "viz-{}",
                snapshot_id
                    .as_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            ),
            WebviewUrl::App(path.into()),
        )
        .title("OpenSpider – Visualisierung")
        .build()
        .map_err(failure)?;
        window.set_focus().map_err(failure)
    }
    fn exit(&self) {
        self.1.set();
        self.0.exit(0);
    }
    fn focus_main(&self) {
        crate::window::focus_main(&self.0);
    }
}
impl<R: Runtime> DesktopActions for Host<R> {
    fn write_clipboard(&self, text: &str) -> Result<(), CmdError> {
        self.0.clipboard().write_text(text).map_err(failure)
    }
    fn reveal_paths(&self, paths: &[PathBuf]) -> Result<(), CmdError> {
        // Tauri setup records the main thread; never wait for ourselves.
        if std::thread::current().id() == self.2 {
            return reveal(paths);
        }
        let paths = paths.to_vec();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.0
            .run_on_main_thread(move || {
                let _ = tx.send(reveal(&paths));
            })
            .map_err(failure)?;
        rx.recv().map_err(failure)?
    }
}
/// Registriert die Hooks erst, sobald Tauri seine Plugins und Fenster angelegt hat.
pub fn install<R: Runtime>(app: &App, handle: &AppHandle<R>, quitting: crate::window::Quitting) {
    let host = Arc::new(Host(handle.clone(), quitting, std::thread::current().id()));
    app.set_window_host(host.clone());
    app.set_desktop_actions(host);
}

#[cfg(target_os = "macos")]
fn reveal(paths: &[PathBuf]) -> Result<(), CmdError> {
    use std::{
        ffi::{c_char, c_void, CString},
        os::unix::ffi::OsStrExt,
    };
    type Obj = *mut c_void;
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Obj;
        fn sel_registerName(name: *const c_char) -> Obj;
        fn objc_msgSend();
    }
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}
    // SAFETY: selectors and receiver classes match the documented Objective-C signatures. All objects
    // live inside the autorelease pool on Tauri's main thread; C strings outlive each synchronous call.
    unsafe {
        let send0: unsafe extern "C" fn(Obj, Obj) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let send1: unsafe extern "C" fn(Obj, Obj, Obj) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let string: unsafe extern "C" fn(Obj, Obj, *const c_char) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let pool = send0(
            objc_getClass(c"NSAutoreleasePool".as_ptr()),
            sel_registerName(c"new".as_ptr()),
        );
        let array = send0(
            objc_getClass(c"NSMutableArray".as_ptr()),
            sel_registerName(c"array".as_ptr()),
        );
        for path in paths {
            let path = match CString::new(path.as_os_str().as_bytes()) {
                Ok(path) => path,
                Err(e) => {
                    send0(pool, sel_registerName(c"drain".as_ptr()));
                    return Err(failure(e));
                }
            };
            let ns = string(
                objc_getClass(c"NSString".as_ptr()),
                sel_registerName(c"stringWithUTF8String:".as_ptr()),
                path.as_ptr(),
            );
            if ns.is_null() {
                send0(pool, sel_registerName(c"drain".as_ptr()));
                return Err(failure("Dateipfad ist kein UTF-8"));
            }
            let url = send1(
                objc_getClass(c"NSURL".as_ptr()),
                sel_registerName(c"fileURLWithPath:".as_ptr()),
                ns,
            );
            send1(array, sel_registerName(c"addObject:".as_ptr()), url);
        }
        let workspace = send0(
            objc_getClass(c"NSWorkspace".as_ptr()),
            sel_registerName(c"sharedWorkspace".as_ptr()),
        );
        send1(
            workspace,
            sel_registerName(c"activateFileViewerSelectingURLs:".as_ptr()),
            array,
        );
        send0(pool, sel_registerName(c"drain".as_ptr()));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn reveal(paths: &[PathBuf]) -> Result<(), CmdError> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr};
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHParseDisplayName(
            name: *const u16,
            bind: *mut c_void,
            pidl: *mut *mut c_void,
            attrs: u32,
            out: *mut u32,
        ) -> i32;
        fn SHOpenFolderAndSelectItems(
            pidl: *const c_void,
            count: u32,
            children: *const *const c_void,
            flags: u32,
        ) -> i32;
    }
    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoTaskMemFree(p: *mut c_void);
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
    }
    // Balance successful COM initialization, including early error returns.
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: this guard exists only after successful initialization on this thread.
            unsafe { CoUninitialize() };
        }
    }
    // SAFETY: null reserved pointer and COINIT_APARTMENTTHREADED (2).
    let initialized = unsafe { CoInitializeEx(ptr::null_mut(), 2) };
    if initialized < 0 {
        return Err(failure(format!("CoInitializeEx: {initialized:#x}")));
    }
    let _apartment = Apartment;
    for path in paths {
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        if wide.contains(&0) {
            return Err(failure("Ungültiger Dateipfad"));
        }
        wide.push(0);
        let mut pidl = ptr::null_mut();
        // SAFETY: wide is NUL terminated; the shell allocates pidl, which is freed exactly once.
        unsafe {
            let parsed = SHParseDisplayName(
                wide.as_ptr(),
                ptr::null_mut(),
                &mut pidl,
                0,
                ptr::null_mut(),
            );
            if parsed < 0 {
                return Err(failure(format!("SHParseDisplayName: {parsed:#x}")));
            }
            let opened = SHOpenFolderAndSelectItems(pidl, 0, ptr::null(), 0);
            CoTaskMemFree(pidl);
            if opened < 0 {
                return Err(failure(format!("SHOpenFolderAndSelectItems: {opened:#x}")));
            }
        }
    }
    Ok(())
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal(_paths: &[PathBuf]) -> Result<(), CmdError> {
    Err(CmdError::new(
        ErrorCode::Unsupported,
        "err-desktop-platform",
    ))
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    // spec: UI-001
    #[test]
    fn reveal_rejects_nul_path_after_com_initialization() {
        let error = reveal(&[PathBuf::from("invalid\0path")]).unwrap_err();
        assert_eq!(error.code, ErrorCode::Io);
    }
}

#[cfg(all(test, not(any(target_os = "macos", target_os = "windows"))))]
mod linux_tests {
    use super::*;

    // spec: UI-001
    #[test]
    fn reveal_is_explicitly_unsupported() {
        assert_eq!(reveal(&[]).unwrap_err().code, ErrorCode::Unsupported);
    }
}
