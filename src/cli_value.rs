use serde::Deserialize;
use serde::Serialize;

/// The type for representing a simple value, used throughout the implementation of the CLI.
/// It implements `serde::{Deserialize, Serialize}` and `rusqlite::ToSql`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CLIValue {
    I64(i64),
    F64(f64),
    Bool(bool),
    Blob(Blob),
    String(String),
    Nil,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Blob(pub Vec<u8>);

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MyBlobVisitor;

        impl serde::de::Visitor<'_> for MyBlobVisitor {
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

impl From<i64> for CLIValue {
    fn from(value: i64) -> Self {
        CLIValue::I64(value)
    }
}

impl From<f64> for CLIValue {
    fn from(value: f64) -> Self {
        CLIValue::F64(value)
    }
}

impl From<bool> for CLIValue {
    fn from(value: bool) -> Self {
        CLIValue::Bool(value)
    }
}

impl From<String> for CLIValue {
    fn from(value: String) -> Self {
        CLIValue::String(value)
    }
}

impl From<&String> for CLIValue {
    fn from(value: &String) -> Self {
        CLIValue::String(value.to_owned())
    }
}

impl From<&str> for CLIValue {
    fn from(value: &str) -> Self {
        CLIValue::String(value.to_owned())
    }
}

impl From<Vec<u8>> for CLIValue {
    fn from(value: Vec<u8>) -> Self {
        CLIValue::Blob(Blob(value))
    }
}

impl<T: Into<CLIValue>> From<Option<T>> for CLIValue {
    fn from(value: Option<T>) -> Self {
        match value {
            None => CLIValue::Nil,
            Some(v) => v.into(),
        }
    }
}

impl From<()> for CLIValue {
    fn from(_value: ()) -> Self {
        CLIValue::Nil
    }
}

impl rusqlite::ToSql for CLIValue {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            CLIValue::I64(value) => value.to_sql(),
            CLIValue::F64(value) => value.to_sql(),
            CLIValue::Bool(value) => value.to_sql(),
            CLIValue::String(value) => value.to_sql(),
            CLIValue::Blob(value) => value.0.to_sql(),
            CLIValue::Nil => rusqlite::types::Null.to_sql(),
        }
    }
}

impl<'a> From<rusqlite::types::ValueRef<'a>> for CLIValue {
    fn from(value: rusqlite::types::ValueRef<'a>) -> Self {
        match value {
            rusqlite::types::ValueRef::Blob(v) => CLIValue::Blob(Blob(v.to_vec())),
            rusqlite::types::ValueRef::Integer(v) => CLIValue::I64(v),
            rusqlite::types::ValueRef::Null => CLIValue::Nil,
            rusqlite::types::ValueRef::Real(v) => CLIValue::F64(v),
            rusqlite::types::ValueRef::Text(v) => CLIValue::String(String::from_utf8_lossy(v).to_string()),
        }
    }
}

#[cfg(test)]
mod test_encode {
    use super::Blob;
    use super::CLIValue;

    #[test]
    fn test_string() {
        assert_eq!(
            rmp_serde::to_vec(&CLIValue::String("a".to_owned())).unwrap(),
            vec![0xa1, 0x61] // 0xa1 = fixstr len=1
        );
    }

    #[test]
    fn test_blob() {
        assert_eq!(
            rmp_serde::to_vec(&CLIValue::Blob(Blob(vec![0xff, 0xef, 0xdf]))).unwrap(),
            vec![0xc4, 0x03, 0xff, 0xef, 0xdf] // 0xc4 0x03 = bin 8 len=3
        );
    }
}

#[cfg(test)]
mod test_decode {
    use super::Blob;
    use super::CLIValue;

    #[test]
    fn test_string() {
        assert_eq!(
            rmp_serde::from_slice::<CLIValue>(&[0xa1, 0x61]).unwrap(), // 0xa1 = fixstr len=1
            CLIValue::String("a".to_owned())
        );
    }

    #[test]
    fn test_blob() {
        assert_eq!(
            rmp_serde::from_slice::<CLIValue>(&[0xc4, 0x03, 0xff, 0xef, 0xdf]).unwrap(), // 0xc4 0x03 = bin 8 len=3
            CLIValue::Blob(Blob(vec![0xff, 0xef, 0xdf]))
        );
    }

    #[test]
    fn test_uint64() {
        assert_eq!(
            rmp_serde::from_slice::<CLIValue>(&[0xcf, 0x0, 0x2b, 0xdc, 0x54, 0x5d, 0x6b, 0x4b, 0x87]).unwrap(),
            CLIValue::I64(12345678901234567)
        );
    }

    #[test]
    fn test_int64() {
        assert_eq!(
            rmp_serde::from_slice::<CLIValue>(&[0xd3, 0xff, 0xd4, 0x23, 0xab, 0xa2, 0x94, 0xb4, 0x79]).unwrap(),
            CLIValue::I64(-12345678901234567)
        );
    }

    #[test]
    fn test_float64() {
        assert_eq!(
            rmp_serde::from_slice::<CLIValue>(&[0xcb, 0xc0, 0x5e, 0xdd, 0x3b, 0xe2, 0x2e, 0x5d, 0xe1]).unwrap(),
            CLIValue::F64(-123.45678)
        );
    }
}

#[cfg(test)]
mod test_from_primitive {
    use super::Blob;
    use super::CLIValue;

    #[test]
    fn test_from_primitive() {
        assert_eq!(Into::<CLIValue>::into(0), CLIValue::I64(0));
        assert_eq!(Into::<CLIValue>::into(1.2), CLIValue::F64(1.2));
        assert_eq!(Into::<CLIValue>::into(false), CLIValue::Bool(false));
        assert_eq!(Into::<CLIValue>::into("a".to_owned()), CLIValue::String("a".to_owned()));
        assert_eq!(Into::<CLIValue>::into("a"), CLIValue::String("a".to_owned()));
        assert_eq!(Into::<CLIValue>::into(vec![1, 2]), CLIValue::Blob(Blob(vec![1, 2])));
        assert_eq!(Into::<CLIValue>::into(()), CLIValue::Nil);
    }

    #[test]
    fn test_value() {
        use std::collections::HashMap;
        let value: HashMap<&str, CLIValue> = serde_json::from_str(r#"{"a": 10, "b": null}"#).unwrap();
        assert_eq!(value, HashMap::from([("a", CLIValue::I64(10)), ("b", CLIValue::Nil)]));
    }
}
