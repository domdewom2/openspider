//! search.* (UI-011): derselbe Parser und Regex-Dialekt wie in Views und Exporten.
use crate::dto::{self, CmdError, RuleBool, RuleNode, RuleOp, SearchParseArgs, ValidateRegexArgs};
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{parse_args, to_json};
use crate::runtime::{App, CallCtx};
use futures_util::future::BoxFuture;
use os_analyze::search as core;

/// Der DTO-Vertrag kennt Gruppen und Bedingungen. NOT wird ohne neue DTO-Variante über De Morgan abgebildet.
fn dto_tree(tree: &core::RuleNode, negate: bool) -> RuleNode {
    match tree {
        core::RuleNode::Not(child) => dto_tree(child, !negate),
        core::RuleNode::And(children) | core::RuleNode::Or(children) => {
            let and = matches!(tree, core::RuleNode::And(_)) ^ negate;
            RuleNode::Group {
                op: if and { RuleBool::And } else { RuleBool::Or },
                children: children.iter().map(|c| dto_tree(c, negate)).collect(),
            }
        }
        core::RuleNode::Rule(rule) => {
            use core::RuleOp as Op;
            let op = match rule.op {
                Op::Contains => RuleOp::Contains,
                Op::NotContains => RuleOp::NotContains,
                Op::Equals => RuleOp::Equals,
                Op::NotEquals => RuleOp::NotEquals,
                Op::StartsWith => RuleOp::StartsWith,
                Op::EndsWith => RuleOp::EndsWith,
                Op::Regex => RuleOp::Regex,
                Op::NotRegex => RuleOp::NotRegex,
                Op::Greater => RuleOp::GreaterThan,
                Op::Less => RuleOp::LessThan,
                Op::IsEmpty => RuleOp::IsEmpty,
                Op::IsNotEmpty => RuleOp::IsNotEmpty,
                Op::GreaterOrEqual | Op::LessOrEqual => {
                    let compare = if rule.op == Op::GreaterOrEqual {
                        Op::Greater
                    } else {
                        Op::Less
                    };
                    let children = [compare, Op::Equals]
                        .map(|op| {
                            dto_tree(
                                &core::RuleNode::Rule(core::Rule { op, ..rule.clone() }),
                                negate,
                            )
                        })
                        .to_vec();
                    return RuleNode::Group {
                        op: if negate { RuleBool::And } else { RuleBool::Or },
                        children,
                    };
                }
            };
            let column = if rule.column.is_empty() || rule.column == "*" {
                None
            } else {
                Some(rule.column.clone())
            };
            let value =
                (!matches!(op, RuleOp::IsEmpty | RuleOp::IsNotEmpty)).then(|| rule.value.clone());
            let condition = RuleNode::Cond { column, op, value };
            if negate {
                negate_dto(condition)
            } else {
                condition
            }
        }
    }
}

fn negate_dto(node: RuleNode) -> RuleNode {
    let RuleNode::Cond { column, op, value } = node else {
        unreachable!()
    };
    let opposite = match op {
        RuleOp::Contains => RuleOp::NotContains,
        RuleOp::NotContains => RuleOp::Contains,
        RuleOp::Equals => RuleOp::NotEquals,
        RuleOp::NotEquals => RuleOp::Equals,
        RuleOp::Regex => RuleOp::NotRegex,
        RuleOp::NotRegex => RuleOp::Regex,
        RuleOp::IsEmpty => RuleOp::IsNotEmpty,
        RuleOp::IsNotEmpty => RuleOp::IsEmpty,
        // NOT(x > n) schließt leere Zellen ein: (< n OR = n OR IS EMPTY).
        RuleOp::GreaterThan | RuleOp::LessThan => {
            let compare = if op == RuleOp::GreaterThan {
                RuleOp::LessThan
            } else {
                RuleOp::GreaterThan
            };
            return RuleNode::Group {
                op: RuleBool::Or,
                children: vec![
                    RuleNode::Cond {
                        column: column.clone(),
                        op: compare,
                        value: value.clone(),
                    },
                    RuleNode::Cond {
                        column: column.clone(),
                        op: RuleOp::Equals,
                        value,
                    },
                    RuleNode::Cond {
                        column,
                        op: RuleOp::IsEmpty,
                        value: None,
                    },
                ],
            };
        }
        // Der bestehende DTO-Vertrag hat keine negierten Affixoperatoren. Der Parser
        // weist diese optionale NOT-Form vor der Umwandlung zurück.
        RuleOp::StartsWith | RuleOp::EndsWith => unreachable!("checked by dto_supported"),
    };
    RuleNode::Cond {
        column,
        op: opposite,
        value,
    }
}

/// DTO → Kern für die View-Services; `None` als Spalte bedeutet alle sichtbaren Spalten.
pub fn to_core(tree: &RuleNode) -> core::RuleNode {
    match tree {
        RuleNode::Group { op, children } => {
            let nodes = children.iter().map(to_core).collect();
            match op {
                RuleBool::And => core::RuleNode::And(nodes),
                RuleBool::Or => core::RuleNode::Or(nodes),
            }
        }
        RuleNode::Cond { column, op, value } => core::RuleNode::Rule(core::Rule {
            column: column.clone().unwrap_or_default(),
            value: value.clone().unwrap_or_default(),
            op: match op {
                RuleOp::Contains => core::RuleOp::Contains,
                RuleOp::NotContains => core::RuleOp::NotContains,
                RuleOp::Equals => core::RuleOp::Equals,
                RuleOp::NotEquals => core::RuleOp::NotEquals,
                RuleOp::StartsWith => core::RuleOp::StartsWith,
                RuleOp::EndsWith => core::RuleOp::EndsWith,
                RuleOp::Regex => core::RuleOp::Regex,
                RuleOp::NotRegex => core::RuleOp::NotRegex,
                RuleOp::GreaterThan => core::RuleOp::Greater,
                RuleOp::LessThan => core::RuleOp::Less,
                RuleOp::IsEmpty => core::RuleOp::IsEmpty,
                RuleOp::IsNotEmpty => core::RuleOp::IsNotEmpty,
            },
        }),
    }
}

// UI-011 verlangt AND/OR; zusätzliche NOT-Syntax nur bei verlustfreier DTO-Darstellung.
fn dto_supported(tree: &core::RuleNode, negate: bool) -> bool {
    match tree {
        core::RuleNode::Not(child) => dto_supported(child, !negate),
        core::RuleNode::And(children) | core::RuleNode::Or(children) => {
            children.iter().all(|c| dto_supported(c, negate))
        }
        core::RuleNode::Rule(rule) => {
            let affix = matches!(rule.op, core::RuleOp::StartsWith | core::RuleOp::EndsWith);
            let all_columns = rule.column.is_empty() || rule.column == "*";
            let comparison = matches!(
                rule.op,
                core::RuleOp::Greater
                    | core::RuleOp::Less
                    | core::RuleOp::GreaterOrEqual
                    | core::RuleOp::LessOrEqual
            );
            !(negate && (affix || (all_columns && comparison)))
        }
    }
}

fn parse<'a>(_app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let SearchParseArgs { text } = parse_args(args)?;
        if ctx.cancel.is_cancelled() {
            return Err(CmdError::new(dto::ErrorCode::Cancelled, "err-cancelled"));
        }
        let tree = core::parse(&text).map_err(|e| {
            CmdError::invalid_input("err-search-syntax", "text")
                .with_arg("position", e.position)
                .with_arg("reason", e.code)
        })?;
        if !dto_supported(&tree, false) {
            return Err(CmdError::invalid_input("err-search-syntax", "text")
                .with_arg("position", 0)
                .with_arg("reason", "negation-not-representable"));
        }
        to_json(&dto_tree(&tree, false))
    })
}
fn validate<'a>(_app: &'a App, ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let ValidateRegexArgs { pattern } = parse_args(args)?;
        if ctx.cancel.is_cancelled() {
            return Err(CmdError::new(dto::ErrorCode::Cancelled, "err-cancelled"));
        }
        // Regex-Kompilierung kann Arbeit kosten und gehört nicht auf einen Tokio-Worker.
        tokio::task::spawn_blocking(move || core::validate_regex(&pattern))
            .await
            .map_err(|e| {
                CmdError::new(dto::ErrorCode::Internal, "err-internal").with_detail(e.to_string())
            })?
            .map_err(|e| {
                CmdError::invalid_input("err-search-regex", "pattern").with_detail(e.to_string())
            })?;
        Ok(serde_json::Value::Null)
    })
}

/// Registriert die unveränderten Katalogverträge.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "search.parse",
            args: "SearchParseArgs",
            ret: "RuleNode",
            long_running: false,
        },
        parse,
    );
    r.register(
        CommandMeta {
            name: "search.validateRegex",
            args: "ValidateRegexArgs",
            ret: "void",
            long_running: false,
        },
        validate,
    );
}
