use super::cli_value::CLIValue;

#[derive(ts_rs::TS, Clone, Debug, Eq, PartialEq)]
#[ts(export)]
pub enum CLIErrorCode {
    PermissionDenied,
    DatabaseBusy,
    DatabaseLocked,
    ReadOnly,
    SystemIoFailure,
    DatabaseCorrupt,
    DiskFull,
    NotADatabase,
    OtherError,

    Success,
}

/// The standardized stderr message format for the CLI.
#[cfg_attr(test, derive(Debug))]
#[derive(Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum CLIError {
    Query {
        message: String,
        query: String,
        params: Vec<CLIValue>,
        code: CLIErrorCode,
    },
    FFI {
        message: String,
        function_name: String,
        params: Vec<CLIValue>,
    },
    UnexpectedChanges {
        expected: u64,
        actual: u64,
        query: String,
        params: Vec<CLIValue>,
    },
    InvalidNumberOfParameters {
        query: String,
        placeholders: Vec<Option<String>>,
        params: Vec<CLIValue>,
    },
    Other {
        message: String,
        query: Option<String>,
        params: Option<Vec<CLIValue>>,
    },
}

impl CLIError {
    pub fn new_query_error<T, U: Into<String>>(
        err: rusqlite::Error,
        query: U,
        params: &[CLIValue],
    ) -> std::result::Result<T, Self> {
        Err(Self::Query {
            message: format!("{err}"),
            query: query.into(),
            params: params.to_vec(),
            code: match err {
                rusqlite::Error::SqliteFailure(rusqlite::ffi::Error { code, .. }, ..) => match code {
                    rusqlite::ffi::ErrorCode::PermissionDenied => CLIErrorCode::PermissionDenied,
                    rusqlite::ffi::ErrorCode::DatabaseBusy => CLIErrorCode::DatabaseBusy,
                    rusqlite::ffi::ErrorCode::DatabaseLocked => CLIErrorCode::DatabaseLocked,
                    rusqlite::ffi::ErrorCode::ReadOnly => CLIErrorCode::ReadOnly,
                    rusqlite::ffi::ErrorCode::SystemIoFailure => CLIErrorCode::SystemIoFailure,
                    rusqlite::ffi::ErrorCode::DatabaseCorrupt => CLIErrorCode::DatabaseCorrupt,
                    rusqlite::ffi::ErrorCode::DiskFull => CLIErrorCode::DiskFull,
                    rusqlite::ffi::ErrorCode::NotADatabase => CLIErrorCode::NotADatabase,
                    _ => CLIErrorCode::OtherError,
                },
                _ => CLIErrorCode::OtherError,
            },
        })
    }

    pub fn new_ffi_error<T, U: Into<String>>(
        err: rusqlite::Error,
        function_name: U,
        params: &[CLIValue],
    ) -> std::result::Result<T, Self> {
        Err(Self::FFI {
            message: format!("{err}"),
            function_name: function_name.into(),
            params: params.to_vec(),
        })
    }

    pub fn new_other_error<T, U: Into<String>>(
        msg: U,
        query: Option<String>,
        params: Option<&[CLIValue]>,
    ) -> std::result::Result<T, Self> {
        Err(Self::Other {
            message: msg.into(),
            query,
            params: params.map(|v| v.into()),
        })
    }

    fn format_query(query: &str) -> String {
        if let Some(query_stripped) = query.strip_prefix("EDITOR_PRAGMA ") {
            format!("Method: {query_stripped}")
        } else {
            format!("Query: {query}")
        }
    }

    fn format_params(params: &[CLIValue]) -> String {
        serde_json::to_string(&params).unwrap_or("<failed to serialize>".to_owned())
    }

    pub fn code(&self) -> CLIErrorCode {
        match self {
            Self::Query { code, .. } => code.to_owned(),
            _ => CLIErrorCode::OtherError,
        }
    }
}

impl std::fmt::Display for CLIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Query {
                message, query, params, ..
            } => {
                write!(
                    f,
                    "{message}\n{}\nParameters: {}",
                    Self::format_query(query),
                    Self::format_params(params)
                )
            }
            Self::FFI {
                message,
                function_name,
                params,
            } => {
                write!(
                    f,
                    "{message}\nFunction: {function_name}\nParameters: {}",
                    Self::format_params(params)
                )
            }
            Self::UnexpectedChanges {
                expected,
                actual,
                query,
                params,
            } => {
                if *expected > 0 && *actual == 0 {
                    write!(
                        f,
                        "The query did not make any changes. Maybe the data was already updated by a different process.\n{}\nParameters: {}",
                        Self::format_query(query),
                        Self::format_params(params),
                    )
                } else {
                    write!(
                        f,
                        "Rolled back the transaction because an unexpected number of rows were modified: expected {expected} rows, actually modified {actual} rows.\n{}\nParameters: {}",
                        Self::format_query(query),
                        Self::format_params(params),
                    )
                }
            }
            Self::InvalidNumberOfParameters {
                query,
                placeholders,
                params,
            } => {
                write!(
                    f,
                    "Invalid number of parameters.\n{}\nParameters: {}\nPlaceholders: [{}]",
                    Self::format_query(query),
                    Self::format_params(params),
                    placeholders
                        .iter()
                        .map(|v| v.clone().unwrap_or("?".to_owned()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Other { message, query, params } => {
                write!(
                    f,
                    "{message}{}{}",
                    query
                        .as_ref()
                        .map(|query| Self::format_query(query))
                        .unwrap_or_default(),
                    params
                        .as_ref()
                        .map(|params| Self::format_params(params))
                        .unwrap_or_default()
                )
            }
        }
    }
}

impl From<csv::Error> for CLIError {
    fn from(value: csv::Error) -> Self {
        Self::Other {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<std::io::Error> for CLIError {
    fn from(value: std::io::Error) -> Self {
        Self::Other {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<serde_json::Error> for CLIError {
    fn from(value: serde_json::Error) -> Self {
        Self::Other {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<rmp_serde::encode::Error> for CLIError {
    fn from(value: rmp_serde::encode::Error) -> Self {
        Self::Other {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<rust_xlsxwriter::XlsxError> for CLIError {
    fn from(value: rust_xlsxwriter::XlsxError) -> Self {
        Self::Other {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}
