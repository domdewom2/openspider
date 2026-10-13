//! Zellwert für Views, Exporte und MCP.

use serde::{Deserialize, Serialize};

/// Ein Zellwert einer Tabellenansicht.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// Leere Zelle.
    Null,
    /// Wahrheitswert.
    Bool(bool),
    /// Ganzzahl.
    Int(i64),
    /// Gleitkommazahl.
    Float(f64),
    /// Text.
    Text(String),
}

impl Value {
    /// `true` bei `Null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Value::Null, Into::into)
    }
}
