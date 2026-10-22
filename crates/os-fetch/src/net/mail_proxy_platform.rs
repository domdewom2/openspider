//! Native system proxy lookup for non-HTTP transports; PAC remains P2 (CRAWL-122).
#[cfg(target_os = "macos")]
pub fn resolve() -> Option<String> {
    use std::{
        ffi::{c_char, c_void, CString},
        ptr,
    };
    type Ref = *const c_void;
    #[link(name = "CFNetwork", kind = "framework")]
    unsafe extern "C" {
        fn CFNetworkCopySystemProxySettings() -> Ref;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(a: Ref, s: *const c_char, e: u32) -> Ref;
        fn CFDictionaryGetValue(d: Ref, k: Ref) -> Ref;
        fn CFNumberGetValue(n: Ref, t: i32, v: *mut i32) -> bool;
        fn CFStringGetCString(s: Ref, b: *mut c_char, n: isize, e: u32) -> bool;
        fn CFRelease(r: Ref);
    }
    // CFNetwork returns a retained dictionary; values are borrowed until CFRelease.
    unsafe {
        let dict = CFNetworkCopySystemProxySettings();
        if dict.is_null() {
            return None;
        }
        let get = |key: &str| {
            let key = CString::new(key).ok()?;
            let cf = CFStringCreateWithCString(ptr::null(), key.as_ptr(), 0x08000100);
            if cf.is_null() {
                return None;
            }
            let v = CFDictionaryGetValue(dict, cf);
            CFRelease(cf);
            (!v.is_null()).then_some(v)
        };
        let number = |key: &str| {
            let value = get(key)?;
            let mut out = 0;
            CFNumberGetValue(value, 3, &mut out).then_some(out)
        };
        let string = |key: &str| {
            let value = get(key)?;
            let mut buffer = [0i8; 2048];
            if !CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                0x08000100,
            ) {
                return None;
            }
            Some(
                std::ffi::CStr::from_ptr(buffer.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        let mut out = None;
        for (prefix, scheme) in [("SOCKS", "socks5"), ("HTTPS", "http"), ("HTTP", "http")] {
            if number(&format!("{prefix}Enable")) == Some(1) {
                if let (Some(host), Some(port)) = (
                    string(&format!("{prefix}Proxy")),
                    number(&format!("{prefix}Port")),
                ) {
                    out = Some(format!("{scheme}://{host}:{port}"));
                    break;
                }
            }
        }
        CFRelease(dict);
        out
    }
}
#[cfg(target_os = "windows")]
pub fn resolve() -> Option<String> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Config {
        detect: i32,
        pac: *mut u16,
        proxy: *mut u16,
        bypass: *mut u16,
    }
    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpGetIEProxyConfigForCurrentUser(config: *mut Config) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalFree(ptr: *mut c_void) -> *mut c_void;
    }
    unsafe {
        let mut c = Config {
            detect: 0,
            pac: std::ptr::null_mut(),
            proxy: std::ptr::null_mut(),
            bypass: std::ptr::null_mut(),
        };
        if WinHttpGetIEProxyConfigForCurrentUser(&mut c) == 0 {
            return None;
        }
        let out = if c.proxy.is_null() {
            None
        } else {
            let mut len = 0;
            while *c.proxy.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(c.proxy, len));
            s.split(';').find_map(|p| {
                let p = p.trim();
                if let Some(v) = p.strip_prefix("socks=") {
                    Some(format!("socks5://{v}"))
                } else if let Some(v) = p.strip_prefix("https=").or_else(|| p.strip_prefix("http="))
                {
                    Some(format!("http://{v}"))
                } else if !p.contains('=') {
                    Some(format!("http://{p}"))
                } else {
                    None
                }
            })
        };
        for p in [c.pac, c.proxy, c.bypass] {
            if !p.is_null() {
                GlobalFree(p.cast());
            }
        }
        out
    }
}
