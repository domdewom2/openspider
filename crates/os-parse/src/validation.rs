//! Strukturprüfung des Original-HTML mit dem HTML5-Tokenizer (TECH-008, TECH-280).
//! Kein reparierter DOM und keine Speicherung des Quelltexts. Größen kommen separat
//! aus `ResponseInput`, damit Zeichendekodierung und Validierungsschalter sie nicht verändern.

use std::cell::RefCell;
use std::collections::BTreeSet;

use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
};
use os_model::{ExtKey, ParsedPage, ValidationFacts, ValidationIssueFact, Value};

use crate::PageCtx;

fn head_element(name: &str) -> bool {
    matches!(
        name,
        "title" | "meta" | "link" | "script" | "style" | "base" | "noscript" | "template"
    )
}

#[derive(Default)]
struct Scan {
    seen: BTreeSet<(&'static str, Option<String>)>,
    facts: ValidationFacts,
    index: u64,
    in_head: bool,
    noscript: u32,
    template: u32,
    raw: Option<String>,
    before_head: bool,
    before_html: Option<String>,
}

impl Scan {
    fn issue(&mut self, code: &'static str, element: Option<&str>, in_head: bool) {
        // Ein Befund je Code/Element reicht auch bei langen Textläufen.
        if !self.seen.insert((code, element.map(str::to_owned))) {
            return;
        }
        self.facts.issues.push(ValidationIssueFact {
            code: code.into(),
            element: element.map(str::to_owned),
            in_head,
        });
        if in_head {
            self.facts.head_errors += 1;
        } else {
            self.facts.body_errors += 1;
        }
    }

    fn content(&mut self, element: &str) {
        if self.facts.tokens.head_count == 0 {
            self.before_head = true;
        }
        if self.facts.tokens.html_count == 0 && !head_element(element) {
            self.before_html.get_or_insert_with(|| element.to_owned());
        }
    }

    fn start(&mut self, name: &str) {
        if self.in_head
            && name != "body"
            && (!head_element(name)
                || (self.noscript > 0 && !matches!(name, "link" | "meta" | "style")))
        {
            self.issue("invalid-head-element", Some(name), true);
        }
        if self.facts.tokens.html_count > 0 && self.facts.tokens.first_element_after_html.is_none()
        {
            self.facts.tokens.first_element_after_html = Some(name.to_owned());
        }
        match name {
            "html" => {
                self.facts.tokens.html_count += 1;
                if self.facts.tokens.first_html.is_none() {
                    self.facts.tokens.first_html = Some(self.index);
                    // TECH-283 beginnt nach dem ersten expliziten html-Tag.
                    self.before_head = false;
                    if let Some(element) = self.before_html.clone() {
                        self.issue("body-before-html", Some(&element), false);
                    }
                }
            }
            "head" => {
                self.facts.tokens.head_count += 1;
                if self.facts.tokens.first_head.is_none() {
                    self.facts.tokens.first_head = Some(self.index);
                    if self.before_head {
                        self.issue("head-not-first", None, true);
                    }
                } else {
                    self.issue("multiple-head", None, true);
                }
                self.in_head = true;
            }
            "body" => {
                self.content(name);
                self.in_head = false;
                self.noscript = 0;
                self.facts.tokens.body_count += 1;
                if self.facts.tokens.first_body.is_none() {
                    self.facts.tokens.first_body = Some(self.index);
                } else {
                    self.issue("multiple-body", None, false);
                }
            }
            _ => {
                self.content(name);
                if name == "noscript" && self.in_head {
                    self.noscript += 1;
                }
            }
        }
    }
}

#[derive(Default)]
struct Sink(RefCell<Scan>);

impl TokenSink for Sink {
    type Handle = ();

    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        let mut s = self.0.borrow_mut();
        if matches!(token, Token::ParseError(_)) {
            return TokenSinkResult::Continue;
        }
        // Unabhängig von der Bündelung durch den Tokenizer: ein Index je Zeichen,
        // alle anderen Token je ein Index. Dadurch bleiben Positionen reproduzierbar.
        let width = match &token {
            Token::CharacterTokens(text) => text.chars().count() as u64,
            _ => 1,
        };
        let result = match token {
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                if tag.kind == EndTag {
                    if s.raw.as_deref() == Some(name) {
                        s.raw = None;
                    } else if name == "template" {
                        s.template = s.template.saturating_sub(1);
                    } else if s.template == 0 {
                        match name {
                            "head" => {
                                s.in_head = false;
                                s.noscript = 0;
                            }
                            "noscript" => s.noscript = s.noscript.saturating_sub(1),
                            _ => {}
                        }
                    }
                    TokenSinkResult::Continue
                } else if tag.kind == StartTag {
                    if s.template == 0 {
                        s.start(name);
                    }
                    if name == "template" {
                        s.template += 1;
                    }
                    let raw = match name {
                        "script" => Some(RawKind::ScriptData),
                        "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                            Some(RawKind::Rawtext)
                        }
                        "title" | "textarea" => Some(RawKind::Rcdata),
                        _ => None,
                    };
                    if let Some(raw) = raw {
                        s.raw = Some(name.to_owned());
                        TokenSinkResult::RawData(raw)
                    } else if name == "plaintext" {
                        s.raw = Some(name.to_owned());
                        TokenSinkResult::Plaintext
                    } else {
                        TokenSinkResult::Continue
                    }
                } else {
                    TokenSinkResult::Continue
                }
            }
            Token::CharacterTokens(text) if s.raw.is_none() && s.template == 0 => {
                if text
                    .chars()
                    .any(|c| !matches!(c, '\t' | '\n' | '\r' | '\u{c}' | ' ' | '\u{feff}'))
                {
                    s.content("#text");
                    if s.in_head {
                        s.issue("invalid-head-element", Some("#text"), true);
                    }
                }
                TokenSinkResult::Continue
            }
            _ => TokenSinkResult::Continue,
        };
        s.index += width;
        result
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        // TECH-280: CDATA enthält keine zählbaren Tags, auch ohne Tree-Builder.
        true
    }
}

/// Reine Token-Analyse, auch für andere Prüfungen nutzbar (z. B. AMP).
pub fn analyze(html: &str) -> ValidationFacts {
    let tokenizer = Tokenizer::new(Sink::default(), Default::default());
    let input = BufferQueue::default();
    input.push_back(html.into());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let mut scan = tokenizer.sink.0.into_inner();
    if scan.facts.tokens.head_count == 0 {
        scan.issue("missing-head", None, true);
    }
    if scan.facts.tokens.body_count == 0 {
        scan.issue("missing-body", None, false);
    }
    scan.facts
}

/// Wird vom Parser ausschließlich bei eingeschalteter Validierung aufgerufen.
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    page.validation = Some(analyze(ctx.html));
}

/// Originale dekomprimierte Bytezahl vor der Zeichendekodierung (TECH-289).
pub fn set_document_bytes(page: &mut ParsedPage, bytes: u64) {
    if let Some(facts) = &mut page.validation {
        facts.document_bytes = bytes;
    }
}

/// Antwortgröße nach Dekomprimierung, unabhängig von TECH-008. Content-Length
/// ist nur ohne Content-Encoding als unkomprimierte Größe verwendbar.
pub fn response_facts(input: &crate::ResponseInput<'_>) -> Vec<(ExtKey, Value)> {
    let encoded = input.headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("content-encoding") && !value.eq_ignore_ascii_case("identity")
    });
    let bytes = input
        .untruncated_bytes
        .or_else(|| {
            (input.truncated && !encoded)
                .then_some(input.content_length)
                .flatten()
        })
        .unwrap_or(input.body.len() as u64)
        .max(input.body.len() as u64);
    vec![
        (
            ExtKey::from_static("validation.body_bytes"),
            Value::Int(bytes.min(i64::MAX as u64) as i64),
        ),
        (
            ExtKey::from_static("validation.truncated"),
            Value::Bool(input.truncated),
        ),
    ]
}

/// Prüfnachweis und kompakte Fakten, vom Writer beim Neuabruf ersetzt.
pub fn to_ext(facts: &ValidationFacts) -> Vec<(ExtKey, Value)> {
    let t = &facts.tokens;
    let mut out = vec![
        (ExtKey::from_static("validation.checked"), Value::Bool(true)),
        (
            ExtKey::from_static("validation.html_count"),
            Value::Int(t.html_count.into()),
        ),
        (
            ExtKey::from_static("validation.head_count"),
            Value::Int(t.head_count.into()),
        ),
        (
            ExtKey::from_static("validation.body_count"),
            Value::Int(t.body_count.into()),
        ),
    ];
    for (key, value) in [
        ("validation.first_html", t.first_html),
        ("validation.first_head", t.first_head),
        ("validation.first_body", t.first_body),
    ] {
        if let Some(value) = value {
            out.push((
                ExtKey::from_static(key),
                Value::Int(value.min(i64::MAX as u64) as i64),
            ));
        }
    }
    if let Some(element) = &t.first_element_after_html {
        out.push((
            ExtKey::from_static("validation.first_element_after_html"),
            Value::Text(element.clone()),
        ));
    }
    for (key, code) in [
        ("validation.invalid_head_elements", "invalid-head-element"),
        ("validation.head_not_first", "head-not-first"),
        ("validation.body_before_html", "body-before-html"),
    ] {
        let issues: Vec<_> = facts.issues.iter().filter(|i| i.code == code).collect();
        if !issues.is_empty() {
            let elements = issues
                .iter()
                .filter_map(|i| i.element.as_deref())
                .collect::<Vec<_>>()
                .join("; ");
            out.push((
                ExtKey::from_static(key),
                Value::Text(if elements.is_empty() {
                    "true".into()
                } else {
                    elements
                }),
            ));
        }
    }
    out
}
