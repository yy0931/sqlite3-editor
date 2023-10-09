use crate::literal::Literal;

#[derive(ts_rs::TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub enum ErrorCode {
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

#[cfg_attr(test, derive(Debug))]
#[derive(Clone, PartialEq)]
pub enum Error {
    QueryError {
        message: String,
        query: String,
        params: Vec<Literal>,
        code: ErrorCode,
    },
    FFIError {
        message: String,
        function_name: String,
        params: Vec<Literal>,
    },
    OtherError {
        message: String,
        query: Option<String>,
        params: Option<Vec<Literal>>,
    },
}

impl Error {
    pub fn new_query_error<T, U: Into<String>>(
        err: rusqlite::Error,
        query: U,
        params: &[Literal],
    ) -> std::result::Result<T, Self> {
        Err(Self::QueryError {
            message: format!("{err}"),
            query: query.into(),
            params: params.iter().cloned().collect(),
            code: match err {
                rusqlite::Error::SqliteFailure(rusqlite::ffi::Error { code, .. }, ..) => match code {
                    rusqlite::ffi::ErrorCode::PermissionDenied => ErrorCode::PermissionDenied,
                    rusqlite::ffi::ErrorCode::DatabaseBusy => ErrorCode::DatabaseBusy,
                    rusqlite::ffi::ErrorCode::DatabaseLocked => ErrorCode::DatabaseLocked,
                    rusqlite::ffi::ErrorCode::ReadOnly => ErrorCode::ReadOnly,
                    rusqlite::ffi::ErrorCode::SystemIoFailure => ErrorCode::SystemIoFailure,
                    rusqlite::ffi::ErrorCode::DatabaseCorrupt => ErrorCode::DatabaseCorrupt,
                    rusqlite::ffi::ErrorCode::DiskFull => ErrorCode::DiskFull,
                    rusqlite::ffi::ErrorCode::NotADatabase => ErrorCode::NotADatabase,
                    _ => ErrorCode::OtherError,
                },
                _ => ErrorCode::OtherError,
            },
        })
    }

    pub fn new_ffi_error<T, U: Into<String>>(
        err: rusqlite::Error,
        function_name: U,
        params: &[Literal],
    ) -> std::result::Result<T, Self> {
        Err(Self::FFIError {
            message: format!("{err}"),
            function_name: function_name.into(),
            params: params.iter().cloned().collect(),
        })
    }

    pub fn new_other_error<T, U: Into<String>>(
        msg: U,
        query: Option<String>,
        params: Option<&[Literal]>,
    ) -> std::result::Result<T, Self> {
        Err(Self::OtherError {
            message: msg.into(),
            query,
            params: params.map(|v| v.into()),
        })
    }

    fn format_query(query: &str) -> String {
        if query.starts_with("EDITOR_PRAGMA ") {
            format!("Method: {}", &query["EDITOR_PRAGMA ".len()..])
        } else {
            format!("Query: {}", query)
        }
    }

    fn format_params(params: &[Literal]) -> String {
        serde_json::to_string(&params).unwrap_or("<failed to serialize>".to_owned())
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::QueryError { code, .. } => code.to_owned(),
            _ => ErrorCode::OtherError,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueryError {
                message, query, params, ..
            } => {
                write!(
                    f,
                    "{}\n{}\nParameters: {}",
                    message,
                    Self::format_query(query),
                    Self::format_params(params)
                )
            }
            Self::FFIError {
                message,
                function_name,
                params,
            } => {
                write!(
                    f,
                    "{}\nFunction: {}\nParameters: {}",
                    message,
                    function_name,
                    Self::format_params(params)
                )
            }
            Self::OtherError { message, query, params } => {
                write!(
                    f,
                    "{}{}{}",
                    message,
                    query
                        .as_ref()
                        .map(|query| Self::format_query(&query))
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

impl From<csv::Error> for Error {
    fn from(value: csv::Error) -> Self {
        Self::OtherError {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::OtherError {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::OtherError {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(value: rmp_serde::encode::Error) -> Self {
        Self::OtherError {
            message: format!("{value}"),
            query: None,
            params: None,
        }
    }
}
