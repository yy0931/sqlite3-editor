//! Safely extracts UTF-8 strings from a rusqlite `Row` or `&[u8]`, invoking a callback on invalid UTF-8 sequences.

use rusqlite::types::ValueRef;
use rusqlite::Row;
use serde::Deserialize;
use serde::Serialize;

/// Converts a `&[u8]` to a UTF-8 string, invoking a callback on invalid UTF-8 sequences.
pub fn into_utf8_lossy<F: FnMut(InvalidUTF8)>(t: &[u8], mut on_invalid_utf8: F) -> String {
    match String::from_utf8(t.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            let text_lossy = String::from_utf8_lossy(t).to_string();
            on_invalid_utf8(InvalidUTF8 {
                text_lossy: text_lossy.clone(),
                bytes: hex::encode(t),
                context: None,
            });
            text_lossy
        }
    }
}

/// Extracts a UTF-8 string value from a `rusqlite::Row`, invoking a callback on invalid UTF-8 sequences.
pub fn get_utf8_string<F: FnMut(InvalidUTF8)>(row: &Row, idx: usize, on_invalid_utf8: F) -> rusqlite::Result<String> {
    let value = row.get_ref(idx)?;
    match value {
        ValueRef::Text(t) => Ok(into_utf8_lossy(t, on_invalid_utf8)),
        value => Err(rusqlite::Error::FromSqlConversionFailure(
            idx,
            value.data_type(),
            Box::new(StringError(format!("Expected a Text but got {value:?}."))),
        )),
    }
}

/// Extracts an optional UTF-8 string (`Option<String>`) value from a `rusqlite::Row`, invoking a callback on invalid UTF-8 sequences.
pub fn get_utf8_string_optional<F: FnMut(InvalidUTF8)>(
    row: &Row,
    idx: usize,
    on_invalid_utf8: F,
) -> rusqlite::Result<Option<String>> {
    let value = row.get_ref(idx)?;
    match value {
        ValueRef::Null => Ok(None),
        ValueRef::Text(t) => Ok(Some(into_utf8_lossy(t, on_invalid_utf8))),
        value => Err(rusqlite::Error::FromSqlConversionFailure(
            idx,
            value.data_type(),
            Box::new(StringError(format!("Expected a Null or Text but got {value:?}."))),
        )),
    }
}

/// Represents the error information for invalid UTF-8 sequences.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct InvalidUTF8 {
    pub text_lossy: String,
    pub bytes: String,
    pub context: Option<String>,
}

impl InvalidUTF8 {
    pub fn with(self, context: &str) -> Self {
        Self {
            context: Some(context.to_owned()),
            ..self
        }
    }
}

#[derive(Clone, Debug)]
struct StringError(String);

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl std::error::Error for StringError {}

#[cfg(test)]
mod test {
    #[test]
    fn test_from_utf8_lossy() {
        let mut warnings = vec![];
        assert_eq!(
            super::into_utf8_lossy(&[b'a', 255], |err| warnings.push(err)),
            "a\u{FFFD}"
        );
        assert_eq!(
            warnings,
            vec![super::InvalidUTF8 {
                bytes: "61ff".to_owned(),
                text_lossy: "a\u{FFFD}".to_owned(),
                context: None,
            }]
        );
    }
}
