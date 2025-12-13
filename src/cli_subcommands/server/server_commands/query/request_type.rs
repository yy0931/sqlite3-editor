use serde::Deserialize;
use serde::Serialize;

use crate::cli_value::CLIValue;

/// Request body
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(from = "(String, Vec<CLIValue>, QueryMode, QueryOptions)")]
#[serde(into = "(String, Vec<CLIValue>, QueryMode, QueryOptions)")]
pub struct QueryCommandParams {
    pub query: String,
    pub params: Vec<CLIValue>,
    pub mode: QueryMode,
    pub options: QueryOptions,
}

impl From<(String, Vec<CLIValue>, QueryMode, QueryOptions)> for QueryCommandParams {
    fn from(value: (String, Vec<CLIValue>, QueryMode, QueryOptions)) -> Self {
        Self {
            query: value.0,
            params: value.1,
            mode: value.2,
            options: value.3,
        }
    }
}

impl From<QueryCommandParams> for (String, Vec<CLIValue>, QueryMode, QueryOptions) {
    fn from(val: QueryCommandParams) -> Self {
        (val.query, val.params, val.mode, val.options)
    }
}

/// "read_only" | "read_write" | "script"
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub enum QueryMode {
    #[serde(rename = "read_only")]
    ReadOnly,
    #[serde(rename = "read_write")]
    ReadWrite,
    #[serde(rename = "script")]
    Script,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct QueryOptions {
    /// Rolls back the transaction if the number of rows affected by the statement does not match this value.
    pub changes: Option<u64>,
    pub allow_fewer_changes: bool,

    /// A statement that is executed before the main statement. It shares the transaction and the parameters with the main statement, and requires ExecMode::ReadWrite.
    pub pre_stmt: Option<String>,

    /// Checks the list of placeholders returned by list_placeholders.rs.
    pub check_placeholders: Option<Vec<Option<String>>>,
}

#[cfg(test)]
mod test {
    use crate::cli_value::Blob;
    use crate::cli_value::CLIValue;

    use super::QueryCommandParams;
    use super::QueryMode;
    use super::QueryOptions;

    #[test]
    fn test_request_encode_and_decode() {
        let data = QueryCommandParams {
            mode: QueryMode::ReadOnly,
            params: vec![CLIValue::Blob(Blob(vec![1, 2, 3]))],
            query: "query".to_owned(),
            options: QueryOptions::default(),
        };
        let msgpack = rmp_serde::to_vec(&data).unwrap();
        let decoded: QueryCommandParams = rmp_serde::from_slice(&msgpack).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_request_encode_and_decode_with_changes() {
        let data = QueryCommandParams {
            mode: QueryMode::ReadWrite,
            params: vec![CLIValue::Blob(Blob(vec![1, 2, 3]))],
            query: "query".to_owned(),
            options: QueryOptions {
                changes: Some(5),
                ..Default::default()
            },
        };
        let msgpack = rmp_serde::to_vec(&data).unwrap();
        let decoded: QueryCommandParams = rmp_serde::from_slice(&msgpack).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_parse_query() {
        let q: QueryCommandParams =
            serde_json::from_str(r#"["foo", [1, 2], "read_only", {"changes": null, "allow_fewer_changes": false}]"#).unwrap();
        assert_eq!(
            q,
            QueryCommandParams {
                query: "foo".to_owned(),
                params: vec![CLIValue::I64(1), CLIValue::I64(2)],
                mode: QueryMode::ReadOnly,
                options: QueryOptions::default(),
            }
        );
    }

    #[test]
    fn test_parse_query_with_changes() {
        let q: QueryCommandParams =
            serde_json::from_str(r#"["foo", [1, 2], "read_only", {"changes": 10, "allow_fewer_changes": true}]"#).unwrap();
        assert_eq!(
            q,
            QueryCommandParams {
                query: "foo".to_owned(),
                params: vec![CLIValue::I64(1), CLIValue::I64(2)],
                mode: QueryMode::ReadOnly,
                options: QueryOptions {
                    changes: Some(10),
                    allow_fewer_changes: true,
                    ..Default::default()
                },
            }
        );
    }

    #[test]
    fn test_query_mode() {
        let value: QueryMode = serde_json::from_str(r#""read_only""#).unwrap();
        assert_eq!(value, QueryMode::ReadOnly);
        assert_eq!(serde_json::to_string(&QueryMode::ReadWrite).unwrap(), r#""read_write""#);
    }
}
