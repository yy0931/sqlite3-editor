use serde::{Deserialize, Serialize};

use crate::literal::Literal;

/// Request body
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "(String, Vec<Literal>, QueryMode)")]
#[serde(into = "(String, Vec<Literal>, QueryMode)")]
pub struct Request {
    pub query: String,
    pub params: Vec<Literal>,
    pub mode: QueryMode,
}

impl Request {
    #[allow(dead_code)]
    pub fn read_only(query: impl Into<String>, params: Vec<Literal>) -> Self {
        Self {
            query: query.into(),
            params,
            mode: QueryMode::ReadOnly,
        }
    }
    #[allow(dead_code)]
    pub fn read_write(query: impl Into<String>, params: Vec<Literal>) -> Self {
        Self {
            query: query.into(),
            params,
            mode: QueryMode::ReadWrite,
        }
    }
    #[allow(dead_code)]
    pub fn script(query: impl Into<String>, params: Vec<Literal>) -> Self {
        Self {
            query: query.into(),
            params,
            mode: QueryMode::Script,
        }
    }
}

impl From<(String, Vec<Literal>, QueryMode)> for Request {
    fn from(value: (String, Vec<Literal>, QueryMode)) -> Self {
        Self {
            query: value.0,
            params: value.1,
            mode: value.2,
        }
    }
}

impl Into<(String, Vec<Literal>, QueryMode)> for Request {
    fn into(self) -> (String, Vec<Literal>, QueryMode) {
        (self.query, self.params, self.mode)
    }
}

/// "read_only" | "read_write" | "script"
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
pub enum QueryMode {
    #[serde(rename = "read_only")]
    ReadOnly,
    #[serde(rename = "read_write")]
    ReadWrite,
    #[serde(rename = "script")]
    Script,
}
