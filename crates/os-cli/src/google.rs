//! CLI Google target: account validation before crawling and disk-backed table uploads.
use crate::{args::RunArgs, exit::CliError, reporter::Reporter};
use os_integrations::{drive::DriveClient, ApiCtx};
use os_model::{Clock, CrawlConfig, SystemClock};
use os_store::paths::AppPaths;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct GoogleTarget {
    pub account: String,
    pub allow_override: bool,
    pub timestamp: Option<String>,
}
impl GoogleTarget {
    pub fn client(&self, config: &CrawlConfig, paths: &AppPaths) -> Result<DriveClient, CliError> {
        self.client_with_secrets(config, paths, os_fetch::secret_store_from_env())
    }
    pub fn client_with_secrets(
        &self,
        config: &CrawlConfig,
        paths: &AppPaths,
        secrets: std::sync::Arc<dyn os_fetch::SecretStore>,
    ) -> Result<DriveClient, CliError> {
        let network = os_model::settings::AppSettings::load(&paths.settings)
            .map_err(|e| CliError::usage(e.to_string()))?
            .network;
        let ctx = ApiCtx {
            cfg: config.clone(),
            secrets,
            network,
            allow_api_override: self.allow_override,
            quota_ledger: None,
        };
        DriveClient::new(&ctx, &self.account, SystemClock.now()).map_err(|_| CliError::usage(format!(
            "Google-Konto '{}' nicht verbunden – das Konto in der App verbinden (--google-drive-account)", self.account)))
    }
}
pub fn validate(args: &RunArgs, paths: &AppPaths) -> Result<Option<GoogleTarget>, CliError> {
    validate_with_secrets(args, paths, os_fetch::secret_store_from_env())
}

pub fn validate_with_secrets(
    args: &RunArgs,
    paths: &AppPaths,
    secrets: std::sync::Arc<dyn os_fetch::SecretStore>,
) -> Result<Option<GoogleTarget>, CliError> {
    let sheets = args
        .export_format
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("gsheet"));
    if !sheets && args.google_drive_account.is_none() {
        return Ok(None);
    }
    let target = GoogleTarget {
        account: args.google_drive_account.clone().filter(|s| !s.trim().is_empty())
            .ok_or_else(|| CliError::usage("gsheet benötigt --google-drive-account; Google-Konto zuerst in der App verbinden"))?,
        allow_override: args.allow_api_override,
        timestamp: None,
    };
    target.client_with_secrets(&CrawlConfig::default(), paths, secrets)?;
    Ok(Some(target))
}

pub(crate) type Rows =
    Box<dyn Iterator<Item = Result<Vec<os_model::Value>, os_export::ExportError>> + Send>;
pub(crate) fn table(path: &Path) -> Result<os_export::target_gsheet::Table<Rows>, String> {
    let (header, mut input) = os_export::csv::read_table(path).map_err(|e| e.to_string())?;
    let count = input
        .try_fold(0u64, |n, r| r.map(|_| n + 1))
        .map_err(|e| e.to_string())?;
    let (_, input) = os_export::csv::read_table(path).map_err(|e| e.to_string())?;
    let kinds = kinds(&header);
    let rows = input.map(move |row| {
        row.map(|r| {
            r.into_iter()
                .enumerate()
                .map(|(i, value)| {
                    use os_analyze::ColumnKind;
                    use os_model::Value;
                    if value.is_empty() {
                        return Value::Null;
                    }
                    match kinds.get(i) {
                        Some(ColumnKind::Int) => {
                            value.parse().map(Value::Int).unwrap_or(Value::Text(value))
                        }
                        Some(ColumnKind::Float { .. } | ColumnKind::Seconds3) => value
                            .parse()
                            .map(Value::Float)
                            .unwrap_or(Value::Text(value)),
                        Some(ColumnKind::Bool) if value.eq_ignore_ascii_case("true") => {
                            Value::Bool(true)
                        }
                        Some(ColumnKind::Bool) if value.eq_ignore_ascii_case("false") => {
                            Value::Bool(false)
                        }
                        _ => Value::Text(value),
                    }
                })
                .collect()
        })
    });
    Ok(os_export::target_gsheet::Table {
        name: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        header,
        row_count: count,
        rows: Box::new(rows),
    })
}
pub async fn upload(
    client: &DriveClient,
    project: &str,
    crawl: &str,
    paths: &[PathBuf],
    single: bool,
    token: &os_model::CancelToken,
    rep: &Reporter,
) -> Result<usize, String> {
    upload_at(client, project, crawl, paths, single, None, token, rep).await
}

#[allow(clippy::too_many_arguments)]
pub async fn upload_at(
    client: &DriveClient,
    project: &str,
    crawl: &str,
    paths: &[PathBuf],
    single: bool,
    timestamp: Option<&str>,
    token: &os_model::CancelToken,
    rep: &Reporter,
) -> Result<usize, String> {
    let mut groups: Vec<Vec<_>> = Vec::new();
    if single {
        groups.push(paths.to_vec());
    } else {
        groups.extend(paths.iter().map(|p| vec![p.clone()]));
    }
    let mut count = 0;
    for group in groups.into_iter().filter(|g| !g.is_empty()) {
        let tables: Vec<_> = group.iter().map(|p| table(p)).collect::<Result<_, _>>()?;
        let name = if single {
            format!("{crawl}_export")
        } else {
            tables[0].name.clone()
        };
        let mut destination = os_export::target_gsheet::Destination::new(project, crawl, &name);
        destination.subfolder = timestamp.map(str::to_string);
        let result = os_export::target_gsheet::export(client, &destination, tables, token)
            .await
            .map_err(|e| e.to_string())?;
        for warning in result.warnings {
            rep.warn(warning);
        }
        rep.info(format!(
            "Google Sheets: {} ({} Zeilen)",
            result.url, result.rows
        ));
        count += 1;
    }
    Ok(count)
}

pub(crate) fn kinds(header: &[String]) -> Vec<os_analyze::ColumnKind> {
    let tabs = os_analyze::tabs::configured(&CrawlConfig::default());
    header
        .iter()
        .map(|name| {
            tabs.iter()
                .flat_map(|tab| &tab.columns)
                .find(|column| column.key == crate::column_headers::canonical(name))
                .map(|column| column.kind)
                .unwrap_or(os_analyze::ColumnKind::Text)
        })
        .collect()
}
