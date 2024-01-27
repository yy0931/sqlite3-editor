use serde::{Deserialize, Serialize};

use crate::{literal::Literal, sqlite3_driver::QueryOptions};

/// Request body
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "(String, Vec<Literal>, QueryMode, QueryOptions)")]
#[serde(into = "(String, Vec<Literal>, QueryMode, QueryOptions)")]
pub struct Request {
    pub query: String,
    pub params: Vec<Literal>,
    pub mode: QueryMode,
    pub options: QueryOptions,
}

impl Request {
    #[allow(dead_code)]
    pub fn read_only(query: impl Into<String>, params: Vec<Literal>, options: QueryOptions) -> Self {
        Self {
            query: query.into(),
            params,
            mode: QueryMode::ReadOnly,
            options,
        }
    }
    #[allow(dead_code)]
    pub fn read_write(query: impl Into<String>, params: Vec<Literal>, options: QueryOptions) -> Self {
        Self {
            query: query.into(),
            params,
            mode: QueryMode::ReadWrite,
            options,
        }
    }
    #[allow(dead_code)]
    pub fn script(query: impl Into<String>, params: Vec<Literal>, options: QueryOptions) -> Self {
        Self {
            query: query.into(),
            params,
            mode: QueryMode::Script,
            options,
        }
    }
}

impl From<(String, Vec<Literal>, QueryMode, QueryOptions)> for Request {
    fn from(value: (String, Vec<Literal>, QueryMode, QueryOptions)) -> Self {
        Self {
            query: value.0,
            params: value.1,
            mode: value.2,
            options: value.3,
        }
    }
}

impl From<Request> for (String, Vec<Literal>, QueryMode, QueryOptions) {
    fn from(val: Request) -> Self {
        (val.query, val.params, val.mode, val.options)
    }
}

/// "read_only" | "read_write" | "script"
#[derive(ts_rs::TS, Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
#[ts(export)]
pub enum QueryMode {
    #[serde(rename = "read_only")]
    ReadOnly,
    #[serde(rename = "read_write")]
    ReadWrite,
    #[serde(rename = "script")]
    Script,
}
