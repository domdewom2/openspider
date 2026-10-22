//! Native system proxy lookup for non-HTTP transports; PAC remains P2 (CRAWL-122).
/// Native proxy address and exclusions must travel together for non-HTTP transports.
pub(super) struct NativeProxy {
    pub url: String,
    pub bypass: Vec<String>,
}

#[cfg(any(target_os = "windows", test))]
fn from_windows(proxy: &str, bypass: &str) -> Option<NativeProxy> {
    let url = proxy.split(';').find_map(|p| {
        let p = p.trim();
        if let Some(v) = p.strip_prefix("socks=") {
            Some(format!("socks5://{v}"))
        } else if let Some(v) = p.strip_prefix("https=").or_else(|| p.strip_prefix("http=")) {
            Some(format!("http://{v}"))
        } else if !p.is_empty() && !p.contains('=') {
            Some(format!("http://{p}"))
        } else {
            None
        }
    })?;
    Some(NativeProxy {
        url,
        bypass: bypass
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
    })
}

#[cfg(any(target_os = "macos", test))]
fn from_macos(url: String, mut exceptions: Vec<String>, exclude_simple: bool) -> NativeProxy {
    if exclude_simple {
        exceptions.push("<local>".into());
    }
    NativeProxy {
        url,
        bypass: exceptions,
    }
}

#[cfg(target_os = "macos")]
pub fn resolve() -> Option<NativeProxy> {
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
        fn CFArrayGetCount(a: Ref) -> isize;
        fn CFArrayGetValueAtIndex(a: Ref, i: isize) -> Ref;
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
        let mut exceptions = Vec::new();
        if let Some(array) = get("ExceptionsList") {
            for i in 0..CFArrayGetCount(array) {
                let value = CFArrayGetValueAtIndex(array, i);
                let mut buffer = [0i8; 2048];
                if !value.is_null()
                    && CFStringGetCString(
                        value,
                        buffer.as_mut_ptr(),
                        buffer.len() as isize,
                        0x08000100,
                    )
                {
                    exceptions.push(
                        std::ffi::CStr::from_ptr(buffer.as_ptr())
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        let exclude_simple = number("ExcludeSimpleHostnames") == Some(1);
        let out = out.map(|url| from_macos(url, exceptions, exclude_simple));
        CFRelease(dict);
        out
    }
}
#[cfg(target_os = "windows")]
pub fn resolve() -> Option<NativeProxy> {
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
        let string = |p: *mut u16| {
            if p.is_null() {
                return String::new();
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
        };
        let out = from_windows(&string(c.proxy), &string(c.bypass));
        for p in [c.pac, c.proxy, c.bypass] {
            if !p.is_null() {
                GlobalFree(p.cast());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // spec: CRAWL-122 [M2]
    // spec: OUT-269
    #[test]
    fn simulated_windows_settings_retain_bypass() {
        let p = from_windows(
            "https=proxy.test:8080;http=other:80",
            "*.internal.test; <local>;smtp.test",
        )
        .unwrap();
        assert_eq!(p.url, "http://proxy.test:8080");
        assert_eq!(p.bypass, ["*.internal.test", "<local>", "smtp.test"]);
        assert!(from_windows("", "<local>").is_none());
    }
    // spec: CRAWL-122 [M2]
    // spec: OUT-269
    #[test]
    fn simulated_macos_settings_retain_exceptions_and_simple_hosts() {
        let p = from_macos(
            "socks5://proxy.test:1080".into(),
            vec!["*.internal.test".into()],
            true,
        );
        assert_eq!(p.url, "socks5://proxy.test:1080");
        assert_eq!(p.bypass, ["*.internal.test", "<local>"]);
        assert!(from_macos("http://proxy:80".into(), vec![], false)
            .bypass
            .is_empty());
    }
}
