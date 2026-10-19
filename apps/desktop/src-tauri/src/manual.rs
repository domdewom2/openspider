//! Fixed documentation destination, opened by native APIs without spawning a shell.
use tauri::{AppHandle, Runtime};

const MANUAL_URL: &std::ffi::CStr = c"https://github.com/domdewom2/openspider/tree/main/docs";

#[tauri::command]
pub async fn open_manual<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(open());
    })
    .map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}

#[cfg(target_os = "macos")]
fn open() -> Result<(), String> {
    use std::ffi::{c_char, c_void};
    type Obj = *mut c_void;
    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Obj;
        fn sel_registerName(name: *const c_char) -> Obj;
        fn objc_msgSend();
    }
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}
    // SAFETY: documented NSWorkspace/NSURL signatures, fixed ASCII URL, main thread;
    // temporary Objective-C objects are scoped to the autorelease pool.
    let opened = unsafe {
        let send0: unsafe extern "C" fn(Obj, Obj) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let send1: unsafe extern "C" fn(Obj, Obj, Obj) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let string: unsafe extern "C" fn(Obj, Obj, *const c_char) -> Obj =
            std::mem::transmute(objc_msgSend as *const ());
        let open_url: unsafe extern "C" fn(Obj, Obj, Obj) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        let pool = send0(
            objc_getClass(c"NSAutoreleasePool".as_ptr()),
            sel_registerName(c"new".as_ptr()),
        );
        let text = string(
            objc_getClass(c"NSString".as_ptr()),
            sel_registerName(c"stringWithUTF8String:".as_ptr()),
            MANUAL_URL.as_ptr(),
        );
        let url = send1(
            objc_getClass(c"NSURL".as_ptr()),
            sel_registerName(c"URLWithString:".as_ptr()),
            text,
        );
        let workspace = send0(
            objc_getClass(c"NSWorkspace".as_ptr()),
            sel_registerName(c"sharedWorkspace".as_ptr()),
        );
        let opened =
            !url.is_null() && open_url(workspace, sel_registerName(c"openURL:".as_ptr()), url);
        send0(pool, sel_registerName(c"drain".as_ptr()));
        opened
    };
    if opened {
        Ok(())
    } else {
        Err("Could not open documentation URL".into())
    }
}

#[cfg(target_os = "windows")]
fn open() -> Result<(), String> {
    use std::{ffi::c_void, ptr};
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            window: *mut c_void,
            verb: *const u16,
            file: *const u16,
            args: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
    let url: Vec<u16> = MANUAL_URL
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: NUL-terminated fixed URL, null optional parameters, SW_SHOWNORMAL.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            ptr::null(),
            url.as_ptr(),
            ptr::null(),
            ptr::null(),
            1,
        )
    };
    if result > 32 {
        Ok(())
    } else {
        Err(format!("Could not open documentation URL: {result}"))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open() -> Result<(), String> {
    use std::{
        ffi::{c_char, c_int, c_void},
        ptr,
    };
    #[link(name = "gio-2.0")]
    unsafe extern "C" {
        fn g_app_info_launch_default_for_uri(
            uri: *const c_char,
            context: *mut c_void,
            error: *mut *mut c_void,
        ) -> c_int;
    }
    // SAFETY: fixed NUL-terminated URL; optional context and error output are null.
    let opened = unsafe {
        g_app_info_launch_default_for_uri(MANUAL_URL.as_ptr(), ptr::null_mut(), ptr::null_mut())
    };
    if opened != 0 {
        Ok(())
    } else {
        Err("Could not open documentation URL".into())
    }
}
