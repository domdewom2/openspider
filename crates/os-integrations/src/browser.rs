//! Shared desktop browser action using the existing webbrowser dependency.
use std::io;

/// Validates the whole batch before any side effect. Desktop implementations use native APIs.
pub fn open_urls(urls: &[String]) -> io::Result<()> {
    let urls = validated_urls(urls)?;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    for url in urls {
        webbrowser::open(url.as_str())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = urls;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "desktop browser unavailable",
        ))
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Ok(())
}

fn validated_urls(urls: &[String]) -> io::Result<Vec<url::Url>> {
    if urls.is_empty() || urls.len() > 10 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected 1 to 10 URLs",
        ));
    }
    urls.iter()
        .map(|text| {
            let url = url::Url::parse(text)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "expected a web URL",
                ));
            }
            Ok(url)
        })
        .collect()
}
