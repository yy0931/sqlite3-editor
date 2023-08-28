use serde::{Deserialize, Serialize};

/// Literal types that implement serde::{Deserialize, Serialize} and rusqlite::ToSql
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Literal {
    I64(i64),
    F64(f64),
    Bool(bool),
    String(String),
    Blob(Vec<u8>),
    Nil,
}

impl From<i64> for Literal {
    fn from(value: i64) -> Self {
        Literal::I64(value)
    }
}

impl From<f64> for Literal {
    fn from(value: f64) -> Self {
        Literal::F64(value)
    }
}

impl From<bool> for Literal {
    fn from(value: bool) -> Self {
        Literal::Bool(value)
    }
}

impl From<String> for Literal {
    fn from(value: String) -> Self {
        Literal::String(value)
    }
}

impl From<&str> for Literal {
    fn from(value: &str) -> Self {
        Literal::String(value.to_owned())
    }
}

impl From<Vec<u8>> for Literal {
    fn from(value: Vec<u8>) -> Self {
        Literal::Blob(value)
    }
}

impl From<()> for Literal {
    fn from(_value: ()) -> Self {
        Literal::Nil
    }
}

impl rusqlite::ToSql for Literal {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        match self {
            Literal::I64(value) => value.to_sql(),
            Literal::F64(value) => value.to_sql(),
            Literal::Bool(value) => value.to_sql(),
            Literal::String(value) => value.to_sql(),
            Literal::Blob(value) => value.to_sql(),
            Literal::Nil => rusqlite::types::Null.to_sql(),
        }
    }
}

impl<'a> From<rusqlite::types::ValueRef<'a>> for Literal {
    fn from(value: rusqlite::types::ValueRef<'a>) -> Self {
        match value {
            rusqlite::types::ValueRef::Blob(v) => Literal::Blob(v.to_vec()),
            rusqlite::types::ValueRef::Integer(v) => Literal::I64(v),
            rusqlite::types::ValueRef::Null => Literal::Nil,
            rusqlite::types::ValueRef::Real(v) => Literal::F64(v),
            // FIXME: utf-16?
            rusqlite::types::ValueRef::Text(v) => Literal::String(String::from_utf8(v.to_vec()).unwrap()),
        }
    }
}

#[test]
fn test_value() {
    use std::collections::HashMap;
    let value: HashMap<&str, Literal> = serde_json::from_str(r#"{"a": 10, "b": null}"#).unwrap();
    assert_eq!(value, HashMap::from([("a", Literal::I64(10)), ("b", Literal::Nil)]));
}

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

#[test]
fn test_parse_query() {
    let q: Request = serde_json::from_str(r#"["foo", [1, 2], "read_only"]"#).unwrap();
    assert_eq!(
        q,
        Request {
            query: "foo".to_owned(),
            params: vec![Literal::I64(1), Literal::I64(2)],
            mode: QueryMode::ReadOnly
        }
    );
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

#[test]
fn test_query_mode() {
    let value: QueryMode = serde_json::from_str(r#""read_only""#).unwrap();
    assert_eq!(value, QueryMode::ReadOnly);
    assert_eq!(serde_json::to_string(&QueryMode::ReadWrite).unwrap(), r#""read_write""#);
}

pub trait TruncateAll {
    fn truncate_all(&mut self) -> ();
}

impl TruncateAll for std::fs::File {
    fn truncate_all(&mut self) -> () {
        self.set_len(0).expect("Failed to truncate the file.");
    }
}

impl<T> TruncateAll for std::io::Cursor<Vec<T>> {
    fn truncate_all(&mut self) -> () {
        self.get_mut().truncate(0);
    }
}
