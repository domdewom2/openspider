//! SERP presentation uses the core's calibrated font metrics (UI-037/ONPAGE-194).
use crate::dto::*;
use os_model::CrawlConfig;
use os_parse::{pixel, text};

pub fn serp_data(
    data: Option<os_store::queries::detail_ext::SerpData>,
) -> Result<SerpData, CmdError> {
    let data = data.ok_or_else(|| CmdError::new(ErrorCode::NotFound, "err-url-not-found"))?;
    let os_store::queries::detail_ext::SerpData {
        address,
        title,
        description,
        edit,
        favicon,
    } = data;
    let url = os_url::parse_start_url(&address)
        .map_err(|e| CmdError::invalid_input("err-url", "urlId").with_detail(e.to_string()))?;
    let edited = edit.map(|e| SerpEdit {
        title: e.title,
        description: e.description,
        edited_at: e.edited_at,
    });
    let title = edited
        .as_ref()
        .and_then(|e| e.title.clone())
        .or(title)
        .unwrap_or_default();
    let description = edited
        .as_ref()
        .and_then(|e| e.description.clone())
        .or(description)
        .unwrap_or_default();
    // Asset URI is filled by the command with its opaque crawl key.
    Ok(SerpData {
        title,
        description,
        host: url.url.host_str().unwrap_or_default().into(),
        breadcrumb: url
            .url
            .path()
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" › "),
        favicon: favicon.map(|id| id.to_string()),
        edited,
    })
}
fn styled_width(text: &str, size: f64, terms: &[String]) -> u32 {
    pixel::width_px_with_bold_terms(text, size, terms)
}
fn row(text: String, min: u32, max: u32, size: f64, terms: &[String]) -> SerpMeasureRow {
    let px = styled_width(&text, size, terms);
    let cut_at = pixel::truncate_at_with_width(&text, max, |s| styled_width(s, size, terms));
    let preview = match cut_at {
        None => text.clone(),
        Some(_) if styled_width(" …", size, terms) > max => String::new(),
        Some(cut) => format!("{} …", text.chars().take(cut as usize).collect::<String>()),
    };
    SerpMeasureRow {
        min_px: Some(f64::from(min)),
        short: Some(px < min),
        chars: text.chars().count() as u32,
        text: Some(text),
        preview: Some(preview),
        px: f64::from(px),
        max_px: f64::from(max),
        remaining: f64::from(max) - f64::from(px),
        truncated: cut_at.is_some(),
        cut_at,
    }
}
pub fn measure(cfg: &CrawlConfig, args: SerpMeasureArgs) -> Result<SerpMeasure, CmdError> {
    if args.title.len() + args.description.len() + args.prefix.len() > 100_000
        || args.bold_terms.len() > 100
        || args.bold_terms.iter().any(|t| t.len() > 1000)
    {
        return Err(CmdError::invalid_input("err-serp-too-large", "title"));
    }
    let device = match args.device {
        SerpDevice::Desktop => "desktop",
        SerpDevice::Mobile => "mobile",
        SerpDevice::Tablet => "tablet",
    };
    let (title_max, desc_max, title_size, desc_size) = pixel::serp_profile(cfg, device);
    let title = text::normalize(&args.title);
    let description = text::normalize(&format!("{}{}", args.prefix, args.description));
    let terms = args
        .bold_terms
        .iter()
        .map(|t| text::normalize(t))
        .collect::<Vec<_>>();
    Ok(SerpMeasure {
        rows: vec![
            row(title, cfg.title.min_px, title_max, title_size, &terms),
            row(
                description,
                cfg.meta_description.min_px,
                desc_max,
                desc_size,
                &terms,
            ),
        ],
    })
}
