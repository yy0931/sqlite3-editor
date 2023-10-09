use serde::{Deserialize, Serialize};

/// Literal types that implement serde::{Deserialize, Serialize} and rusqlite::ToSql
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Literal {
    I64(i64),
    F64(f64),
    Bool(bool),
    Blob(Blob),
    String(String),
    Nil,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Blob(pub Vec<u8>);

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MyBlobVisitor;

        impl<'de> serde::de::Visitor<'de> for MyBlobVisitor {
            type Value = Blob;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("byte array")
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Blob, E>
            where
                E: serde::de::Error,
            {
                Ok(Blob(value.to_vec()))
            }
        }

        deserializer.deserialize_bytes(MyBlobVisitor)
    }
}

impl Serialize for Blob {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
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

impl From<&String> for Literal {
    fn from(value: &String) -> Self {
        Literal::String(value.to_owned())
    }
}

impl From<&str> for Literal {
    fn from(value: &str) -> Self {
        Literal::String(value.to_owned())
    }
}

impl From<Vec<u8>> for Literal {
    fn from(value: Vec<u8>) -> Self {
        Literal::Blob(Blob(value))
    }
}

impl<T: Into<Literal>> From<Option<T>> for Literal {
    fn from(value: Option<T>) -> Self {
        match value {
            None => Literal::Nil,
            Some(v) => v.into(),
        }
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
            Literal::Blob(value) => value.0.to_sql(),
            Literal::Nil => rusqlite::types::Null.to_sql(),
        }
    }
}

impl<'a> From<rusqlite::types::ValueRef<'a>> for Literal {
    fn from(value: rusqlite::types::ValueRef<'a>) -> Self {
        match value {
            rusqlite::types::ValueRef::Blob(v) => Literal::Blob(Blob(v.to_vec())),
            rusqlite::types::ValueRef::Integer(v) => Literal::I64(v),
            rusqlite::types::ValueRef::Null => Literal::Nil,
            rusqlite::types::ValueRef::Real(v) => Literal::F64(v),
            rusqlite::types::ValueRef::Text(v) => Literal::String(String::from_utf8_lossy(v).to_string()),
        }
    }
}
