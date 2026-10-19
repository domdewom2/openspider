//! Kleine, begrenzte Grammatik: NOT > AND > OR, Klammern, zitierte Spalten/Werte.
use super::{ParseError, Rule, RuleNode, RuleOp};

const MAX_TOKENS: usize = 4096;
const MAX_DEPTH: usize = 64;

#[derive(Debug)]
struct Token {
    text: String,
    position: usize,
    quoted: bool,
}

fn error(position: usize, code: &str) -> ParseError {
    ParseError {
        position,
        code: code.into(),
    }
}

fn lex(text: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let position = i;
        let quoted = matches!(chars[i], '"' | '\'');
        let mut value = String::new();
        if quoted {
            let quote = chars[i];
            i += 1;
            loop {
                let c = *chars
                    .get(i)
                    .ok_or_else(|| error(position, "unclosed-string"))?;
                i += 1;
                if c == quote {
                    break;
                }
                if c == '\\' {
                    let next = *chars
                        .get(i)
                        .ok_or_else(|| error(i - 1, "unclosed-string"))?;
                    // Regex escapes (\1, \d, …) bleiben wörtlich. Nur Quote/Backslash entkommen.
                    if next == quote || next == '\\' {
                        value.push(next);
                        i += 1;
                    } else {
                        value.push(c);
                    }
                } else {
                    value.push(c);
                }
            }
        } else if "()~!=<>^$".contains(chars[i]) {
            value.push(chars[i]);
            i += 1;
            if i < chars.len()
                && (chars[i] == '='
                    || (value == "!" && chars[i] == '~')
                    || (value == "<" && chars[i] == '>'))
            {
                value.push(chars[i]);
                i += 1;
            }
        } else {
            while i < chars.len() && !chars[i].is_whitespace() && !"()~!=<>^$\"'".contains(chars[i])
            {
                value.push(chars[i]);
                i += 1;
            }
        }
        out.push(Token {
            text: value,
            position,
            quoted,
        });
        if out.len() > MAX_TOKENS {
            return Err(error(position, "query-too-complex"));
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
    end: usize,
}
impl Parser {
    fn position(&self) -> usize {
        self.tokens.get(self.at).map_or(self.end, |t| t.position)
    }
    fn eat(&mut self, word: &str) -> bool {
        if self
            .tokens
            .get(self.at)
            .is_some_and(|t| !t.quoted && t.text.eq_ignore_ascii_case(word))
        {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn take(&mut self) -> Result<String, ParseError> {
        let t = self
            .tokens
            .get(self.at)
            .ok_or_else(|| error(self.end, "expected-value"))?;
        if !t.quoted
            && matches!(
                t.text.as_str(),
                "(" | ")" | "~" | "!~" | "=" | "<" | ">" | "!"
            )
        {
            return Err(error(t.position, "expected-value"));
        }
        self.at += 1;
        Ok(t.text.clone())
    }
    fn expr(&mut self, depth: usize) -> Result<RuleNode, ParseError> {
        let mut children = vec![self.and(depth)?];
        while self.eat("OR") {
            children.push(self.and(depth)?);
        }
        Ok(if children.len() == 1 {
            children.remove(0)
        } else {
            RuleNode::Or(children)
        })
    }
    fn and(&mut self, depth: usize) -> Result<RuleNode, ParseError> {
        let mut children = vec![self.atom(depth)?];
        while self.eat("AND") {
            children.push(self.atom(depth)?);
        }
        Ok(if children.len() == 1 {
            children.remove(0)
        } else {
            RuleNode::And(children)
        })
    }
    fn atom(&mut self, depth: usize) -> Result<RuleNode, ParseError> {
        if depth >= MAX_DEPTH {
            return Err(error(self.position(), "query-too-complex"));
        }
        if self.eat("NOT") {
            return Ok(RuleNode::Not(Box::new(self.atom(depth + 1)?)));
        }
        if self.eat("(") {
            let result = self.expr(depth + 1)?;
            if !self.eat(")") {
                return Err(error(self.position(), "expected-close-paren"));
            }
            return Ok(result);
        }
        let column = self.take()?;
        let pos = self.position();
        let op = if self.eat("~") || self.eat("CONTAINS") {
            RuleOp::Contains
        } else if self.eat("!~") || self.eat("NOTCONTAINS") {
            RuleOp::NotContains
        } else if self.eat("=") || self.eat("==") {
            RuleOp::Equals
        } else if self.eat("!=") || self.eat("<>") {
            RuleOp::NotEquals
        } else if self.eat("^=") || self.eat("STARTSWITH") {
            RuleOp::StartsWith
        } else if self.eat("$=") || self.eat("ENDSWITH") {
            RuleOp::EndsWith
        } else if self.eat("REGEX") {
            RuleOp::Regex
        } else if self.eat("NOTREGEX") {
            RuleOp::NotRegex
        } else if self.eat(">") {
            RuleOp::Greater
        } else if self.eat("<") {
            RuleOp::Less
        } else if self.eat(">=") {
            RuleOp::GreaterOrEqual
        } else if self.eat("<=") {
            RuleOp::LessOrEqual
        } else if self.eat("ISEMPTY") {
            RuleOp::IsEmpty
        } else if self.eat("ISNOTEMPTY") {
            RuleOp::IsNotEmpty
        } else if self.eat("IS") {
            let not = self.eat("NOT");
            if !self.eat("EMPTY") {
                return Err(error(self.position(), "expected-empty"));
            }
            if not {
                RuleOp::IsNotEmpty
            } else {
                RuleOp::IsEmpty
            }
        } else {
            return Err(error(pos, "expected-operator"));
        };
        let value = if matches!(op, RuleOp::IsEmpty | RuleOp::IsNotEmpty) {
            String::new()
        } else {
            self.take()?
        };
        Ok(RuleNode::Rule(Rule { column, op, value }))
    }
}

pub(super) fn parse(text: &str) -> Result<RuleNode, ParseError> {
    if text.len() > 262_144 {
        return Err(error(0, "query-too-complex"));
    }
    let mut parser = Parser {
        tokens: lex(text)?,
        at: 0,
        end: text.chars().count(),
    };
    let node = parser.expr(0)?;
    if parser.at != parser.tokens.len() {
        return Err(error(parser.position(), "unexpected-token"));
    }
    Ok(node)
}

/// Kanonische Textsyntax. Alle Namen/Werte sind zitiert; Roundtrip erhält den Baum.
pub fn format(tree: &RuleNode) -> String {
    fn quote(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
    match tree {
        RuleNode::And(nodes) | RuleNode::Or(nodes) => {
            let separator = if matches!(tree, RuleNode::And(_)) {
                " AND "
            } else {
                " OR "
            };
            format!(
                "({})",
                nodes.iter().map(format).collect::<Vec<_>>().join(separator)
            )
        }
        RuleNode::Not(node) => format!("NOT ({})", format(node)),
        RuleNode::Rule(rule) => {
            let op = match rule.op {
                RuleOp::Contains => "~",
                RuleOp::NotContains => "!~",
                RuleOp::Equals => "=",
                RuleOp::NotEquals => "!=",
                RuleOp::StartsWith => "^=",
                RuleOp::EndsWith => "$=",
                RuleOp::Regex => "REGEX",
                RuleOp::NotRegex => "NOTREGEX",
                RuleOp::Greater => ">",
                RuleOp::Less => "<",
                RuleOp::GreaterOrEqual => ">=",
                RuleOp::LessOrEqual => "<=",
                RuleOp::IsEmpty => "IS EMPTY",
                RuleOp::IsNotEmpty => "IS NOT EMPTY",
            };
            let value = if matches!(rule.op, RuleOp::IsEmpty | RuleOp::IsNotEmpty) {
                String::new()
            } else {
                format!(" {}", quote(&rule.value))
            };
            format!("{} {op}{value}", quote(&rule.column))
        }
    }
}
