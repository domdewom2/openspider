use os_model::{
    ConfigIssue, ConfigIssueKind, CrawlConfig, CustomJsConfig, JsSnippet, JsSnippetKind,
};

pub fn snippet_name(snippet: &JsSnippet, index: usize) -> String {
    if snippet.name.trim().is_empty() {
        format!("Snippet {}", index + 1)
    } else {
        snippet.name.trim().to_owned()
    }
}

/// Imported code stays out of the runnable configuration until explicitly accepted.
#[derive(Debug)]
pub struct PendingImport(JsSnippet);
impl PendingImport {
    pub fn new(snippet: JsSnippet) -> Self {
        Self(snippet)
    }
    pub fn preview(&self) -> &JsSnippet {
        &self.0
    }
    pub fn confirm(self, manager: &mut SnippetManager) -> Result<(), String> {
        manager.add(self.0)
    }
}

/// Transactional editing surface for configuration dialogs and other transports.
#[derive(Debug, Clone, Default)]
pub struct SnippetManager {
    config: CustomJsConfig,
}
impl SnippetManager {
    pub fn new(config: CustomJsConfig) -> Result<Self, String> {
        validate(&config)?;
        Ok(Self { config })
    }
    pub fn config(&self) -> &CustomJsConfig {
        &self.config
    }
    /// Editor entry point: extraction without a timeout inherits the rendering
    /// deadline; action without an explicit timeout fails validation.
    pub fn add_code(
        &mut self,
        name: String,
        kind: JsSnippetKind,
        code: String,
        timeout_s: Option<u32>,
    ) -> Result<(), String> {
        self.add(JsSnippet {
            name,
            kind,
            code,
            timeout_s: timeout_s.unwrap_or(0),
            ..Default::default()
        })
    }
    pub fn add(&mut self, snippet: JsSnippet) -> Result<(), String> {
        let mut next = self.config.clone();
        next.snippets.push(snippet);
        validate(&next)?;
        self.config = next;
        Ok(())
    }
    pub fn replace(&mut self, index: usize, snippet: JsSnippet) -> Result<(), String> {
        let mut next = self.config.clone();
        *next
            .snippets
            .get_mut(index)
            .ok_or("Snippet index out of bounds")? = snippet;
        validate(&next)?;
        self.config = next;
        Ok(())
    }
    pub fn remove(&mut self, index: usize) -> Result<(), String> {
        let mut next = self.config.clone();
        if index >= next.snippets.len() {
            return Err("Snippet index out of bounds".into());
        }
        next.snippets.remove(index);
        validate(&next)?;
        self.config = next;
        Ok(())
    }
    pub fn move_to(&mut self, from: usize, to: usize) -> Result<(), String> {
        let mut next = self.config.clone();
        if from >= next.snippets.len() || to >= next.snippets.len() {
            return Err("Snippet index out of bounds".into());
        }
        let snippet = next.snippets.remove(from);
        next.snippets.insert(to, snippet);
        validate(&next)?;
        self.config = next;
        Ok(())
    }
}
fn validate(config: &CustomJsConfig) -> Result<(), String> {
    let cfg = CrawlConfig {
        custom_js: config.clone(),
        ..Default::default()
    };
    let report = cfg.validate();
    match report
        .errors
        .iter()
        .find(|i| i.path.starts_with("custom_js."))
    {
        Some(issue) => Err(issue.message.clone()),
        None => Ok(()),
    }
}

/// The existing config validation surface displays this before a crawl starts.
/// Report conservatively once per config rather than once per URL.
pub fn network_warning(cfg: &CrawlConfig) -> Vec<ConfigIssue> {
    if !cfg
        .custom_js
        .snippets
        .iter()
        .any(|s| s.code.contains("fetch") || s.code.contains("XMLHttpRequest"))
    {
        return Vec::new();
    }
    vec![ConfigIssue {
        code: "custom-js-network".into(), position: None, kind: ConfigIssueKind::Warning,
        path: "custom_js.snippets".into(), line: None, column: None,
        message: "JavaScript-Snippets können Netzwerkanfragen senden und dabei Seitendaten an Dritte übertragen.".into(),
    }]
}
