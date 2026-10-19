//! Zerlegung eines geprüften Bereichs in Token mit Zeichenpositionen (ONPAGE-153/154).
//!
//! Ein Token ist eine Folge ohne Unicode-Leerraum (wie die Wortzählung ONPAGE-141). Der **Kern** eines Tokens ist das
//! Token ohne führende und abschließende Satzzeichen, Klammern und Anführungszeichen. Ein Zeilenumbruch im Text ist
//! eine Blockgrenze (Absatz, Listeneintrag, Überschrift; `content::area_text`). Positionen zählen Unicode-Zeichen ab
//! dem Anfang des Bereichs.

/// Ein Token eines Bereichs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// Das Token, wie es im Text steht.
    pub raw: &'a str,
    /// Erste Zeichenposition des Tokens.
    pub start: usize,
    /// Kern ohne Satzzeichen am Rand (leer bei reinen Satzzeichen-Token).
    pub core: &'a str,
    /// Zeichenposition des Kerns.
    pub core_start: usize,
    /// Erstes Token eines Blocks.
    pub block_start: bool,
}

impl Token<'_> {
    /// Zeichenposition hinter dem Kern.
    pub fn core_end(&self) -> usize {
        self.core_start + self.core.chars().count()
    }

    /// Zeichenposition hinter dem Token.
    pub fn end(&self) -> usize {
        self.start + self.raw.chars().count()
    }

    /// Steht hinter dem Kern ein Satzzeichen oder eine Klammer?
    pub fn has_trailing(&self) -> bool {
        self.core_start + self.core.chars().count() < self.end()
    }

    /// Schließt das Token einen Satz ab (`.`, `!`, `?`, `…` am Ende, schließende Klammern und Anführungszeichen
    /// zählen nicht mit)?
    pub fn ends_sentence(&self) -> bool {
        let trimmed = self
            .raw
            .trim_end_matches(|c: char| CLOSING.contains(c) || c == '\'' || c == '"');
        trimmed.ends_with(['.', '!', '?', '…', '؟', '。', '！', '？'])
    }

    /// Sieht das Token wie eine URL oder E-Mail-Adresse aus?
    pub fn is_url_or_email(&self) -> bool {
        is_url_or_email(self.core)
    }
}

/// Schließende Klammern und Anführungszeichen.
const CLOSING: &str = ")]}»›“”’\"'」』）】";

/// Satzzeichen, Klammern und Anführungszeichen, die den Kern eines Tokens begrenzen.
pub fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '“' | '”'
                | '„'
                | '‚'
                | '‘'
                | '’'
                | '«'
                | '»'
                | '‹'
                | '›'
                | '…'
                | '–'
                | '—'
                | '―'
                | '¿'
                | '¡'
                | '·'
                | '、'
                | '。'
                | '，'
                | '．'
                | '！'
                | '？'
                | '；'
                | '：'
                | '،'
                | '؛'
                | '؟'
                | '「'
                | '」'
                | '『'
                | '』'
                | '（'
                | '）'
                | '【'
                | '】'
                | '•'
        )
}

/// URL (Schema mit `://`, `www.`) oder E-Mail-Adresse.
pub fn is_url_or_email(s: &str) -> bool {
    let lower = s.to_lowercase();
    if lower.contains("://") || lower.starts_with("www.") || lower.starts_with("mailto:") {
        return true;
    }
    match s.split_once('@') {
        Some((local, domain)) => !local.is_empty() && domain.contains('.'),
        None => false,
    }
}

/// Zerlegt `text` in Token.
pub fn tokenize(text: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut block_start = true;
    let mut start: Option<(usize, usize)> = None; // (Byte, Zeichen)
    for (chars, (byte, c)) in text.char_indices().enumerate() {
        if c.is_whitespace() {
            if let Some(from) = start.take() {
                out.push(token(text, from, byte, block_start));
                block_start = false;
            }
            if c == '\n' {
                block_start = true;
            }
        } else if start.is_none() {
            start = Some((byte, chars));
        }
    }
    if let Some(from) = start {
        out.push(token(text, from, text.len(), block_start));
    }
    out
}

/// Token aus `text[from.0..to_byte]` (`from.1` = Zeichenposition).
fn token(text: &str, from: (usize, usize), to_byte: usize, block_start: bool) -> Token<'_> {
    let raw = &text[from.0..to_byte];
    let lead = raw.chars().take_while(|c| is_punct(*c)).count();
    let lead_bytes: usize = raw.chars().take(lead).map(char::len_utf8).sum();
    let core = raw[lead_bytes..].trim_end_matches(is_punct);
    Token {
        raw,
        start: from.1,
        core,
        core_start: from.1 + lead,
        block_start,
    }
}

/// Ein zu prüfendes Wort im Kern eines Tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word<'a> {
    /// Das Wort.
    pub text: &'a str,
    /// Zeichenposition im Bereich.
    pub start: usize,
}

/// Abkürzung mit Punkten (`z.B`, `e.g`, `u.a`): Teile aus höchstens drei Buchstaben.
fn is_dotted_abbreviation(s: &str) -> bool {
    s.contains('.')
        && s.split('.').all(|p| {
            let n = p.chars().count();
            (1..=3).contains(&n) && p.chars().all(char::is_alphabetic)
        })
}

/// Reines Großbuchstaben-Akronym bis 5 Buchstaben (`NASA`, `HTML`).
fn is_short_acronym(s: &str) -> bool {
    let letters: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    !letters.is_empty() && letters.len() <= 5 && letters.iter().all(|c| c.is_uppercase())
}

/// Zu prüfende Wörter eines Tokens (ONPAGE-153): Token mit Ziffern, URLs, E-Mail-Adressen und Abkürzungen mit Punkten
/// entfallen ganz; der Kern wird an Schrägstrichen, Klammern und sonstigen Satzzeichen geteilt (Bindestrich und
/// Apostroph im Wort bleiben); reine Großbuchstaben-Akronyme bis 5 Zeichen und einzelne Zeichen entfallen.
pub fn words<'a>(token: &Token<'a>) -> Vec<Word<'a>> {
    let core = token.core;
    if core.is_empty()
        || core.chars().any(|c| c.is_numeric())
        || token.is_url_or_email()
        || is_dotted_abbreviation(core)
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut part_start: Option<(usize, usize)> = None; // (Byte, Zeichen)
    let separator = |c: char| is_punct(c) && !matches!(c, '-' | '\'' | '’');
    let take = |from: (usize, usize), to: usize, out: &mut Vec<Word<'a>>| {
        let part = &core[from.0..to];
        let lead = part.chars().take_while(|c| is_punct(*c)).count();
        let lead_bytes: usize = part.chars().take(lead).map(char::len_utf8).sum();
        let text = part[lead_bytes..].trim_end_matches(is_punct);
        if text.chars().count() > 1 && !is_short_acronym(text) {
            out.push(Word {
                text,
                start: from.1 + lead,
            });
        }
    };
    for (k, (byte, c)) in core.char_indices().enumerate() {
        if separator(c) {
            if let Some(from) = part_start.take() {
                take(from, byte, &mut out);
            }
        } else if part_start.is_none() {
            part_start = Some((byte, token.core_start + k));
        }
    }
    if let Some(from) = part_start {
        take(from, core.len(), &mut out);
    }
    out
}
