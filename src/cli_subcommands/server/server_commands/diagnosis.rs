use crate::cli_error::CLIError;
use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::split_sqlite_statements;
use crate::cli_subcommands::server::sqlite3_query_parser::tokenize::ZeroIndexedTokenizerError;
use crate::cli_subcommands::server::TruncateAll;
use once_cell::sync::Lazy;
use regex::Regex;
use rmp_serde::encode::write_named;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Write;

pub fn run(r: &mut File, w: &mut File) -> CLIErrorCode {
    // Handle the command
    if let Err(err) = rmp_serde::from_read(r).map(
        |DiagnosisCommandParams { query }: DiagnosisCommandParams| -> std::result::Result<(), CLIError> {
            write_named(w, &diagnose(&query)?)?;
            Ok(())
        },
    ) {
        w.flush().unwrap();
        w.truncate_all();
        write!(w, "{err:?}").unwrap();
        CLIErrorCode::OtherError
    } else {
        w.flush().unwrap();
        CLIErrorCode::Success
    }
}

/// Structure representing a database query
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(from = "(String,)")]
pub struct DiagnosisCommandParams {
    pub query: String,
}

impl From<(String,)> for DiagnosisCommandParams {
    fn from(value: (String,)) -> Self {
        Self { query: value.0 }
    }
}

/// Checks the syntax of a string containing SQL statements.
fn diagnose(sql: &str) -> std::result::Result<Vec<Diagnostic>, CLIError> {
    match split_sqlite_statements(sql) {
        Ok((statements, _)) => {
            let mut conn = rusqlite::Connection::open_in_memory()
                .or_else(|err| CLIError::new_ffi_error(err, "sqlite3_open_v2", &[":memory:".into()]))?;

            let mut errors: Vec<Diagnostic> = vec![];
            for stmt in statements {
                let stmt_str = &stmt.real_text.value;
                let offset_start = stmt.real_text.range.start.offset_at(sql);
                if let Some(err) = diagnose_single_statement(&mut conn, stmt_str, offset_start) {
                    errors.push(err);
                }
            }
            Ok(errors)
        }
        Err(ZeroIndexedTokenizerError { location, message }) => {
            Ok(vec![Diagnostic::error(location.offset_at(sql), message)])
        }
    }
}

/// Checks the syntax of a single SQL statement using a SQLite connection.
pub fn diagnose_single_statement(
    conn: &mut rusqlite::Connection,
    stmt_str: &str,
    offset_start: usize,
) -> Option<Diagnostic> {
    // Ignore empty statements
    if stmt_str.trim().is_empty() || stmt_str.trim() == ";" {
        return None;
    }

    // Ignore statements that contain PRAGMA, QUERY, or EXPLAIN, as they are not safe to combine with EXPLAIN.
    if PRAGMA.is_match(stmt_str) || QUERY.is_match(stmt_str) || EXPLAIN.is_match(stmt_str) {
        return None;
    }

    // Try to prepare the statement
    match conn.prepare(&format!("EXPLAIN {stmt_str}")) {
        // syntax error, unrecognized token, incomplete input
        Err(rusqlite::Error::SqlInputError { sql, offset, msg, .. }) if SQL_INPUT_ERROR_SYNTAX_ERROR.is_match(&msg) => {
            Some(Diagnostic::error(
                (offset_start + loose_byte_to_code_point_index(&sql, offset.try_into().unwrap()))
                    .saturating_sub("EXPLAIN ".len()),
                msg,
            ))
        }

        // unknown table option
        Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if SQLITE_FAILURE_SYNTAX_ERROR.is_match(&msg) => {
            Some(Diagnostic::error(offset_start + stmt_str.chars().count(), msg))
        }
        _ => None,
    }
}

static PRAGMA: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(?s).*\bPRAGMA[^_a-zA-Z0-9]").unwrap());
static QUERY: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(?s).*\bQUERY[^_a-zA-Z0-9]").unwrap());
static EXPLAIN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(?s).*\bEXPLAIN[^_a-zA-Z0-9]").unwrap());

static SQL_INPUT_ERROR_SYNTAX_ERROR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(?s)^(?:near .*: syntax error|unrecognized token:|incomplete input)"#).unwrap());
static SQLITE_FAILURE_SYNTAX_ERROR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)(?s)^(?:unknown table option)"#).unwrap());

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Diagnostic {
    pub possible_causes: Vec<PossibleCause>,
    pub message: String,
    pub severity: Severity,
}

impl Diagnostic {
    fn error(offset: usize, message: impl Into<String>) -> Self {
        Self {
            possible_causes: vec![PossibleCause { offset }],
            message: message.into(),
            severity: Severity::Error,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct PossibleCause {
    #[ts(type = "bigint")]
    pub offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub enum Severity {
    Warning,
    Error,
}

/// Converts a byte index into a code point index within a string, handling cases where the byte index may not
/// align with a character boundary.
///
/// # Arguments
///
/// * `s` - A string slice reference to search within.
/// * `byte_index` - The byte index to convert to a code point index.
///
/// # Return
///
/// Returns the code point index corresponding to the given byte index. If the byte index exceeds the length
/// of the string or falls in the middle of a multi-byte character, it returns the code point index of the next
/// character. If the byte index is beyond the end of the string, it returns the total number of characters in the string.
fn loose_byte_to_code_point_index(s: &str, byte_index: usize) -> usize {
    for (code_point_index_i, (byte_index_i, _)) in s.char_indices().enumerate() {
        if byte_index <= byte_index_i {
            return code_point_index_i;
        }
    }
    s.chars().count()
}

#[cfg(test)]
mod test {
    use super::diagnose;
    use super::Diagnostic;

    #[test]
    fn test_simple_pass() {
        assert_eq!(diagnose("DELETE FROM t"), Ok(vec![]));
    }

    #[test]
    fn test_syntax_error() {
        assert_eq!(
            diagnose("DELETE t"),
            Ok(vec![Diagnostic::error(7, "near \"t\": syntax error")]),
        );
    }

    #[test]
    fn test_unterminated_string_literal() {
        assert_eq!(
            diagnose(r#"SELECT 'a"#),
            Ok(vec![Diagnostic::error(7, "Unterminated string literal")]),
        );
    }

    #[test]
    fn test_multiple_errors() {
        assert_eq!(
            diagnose("foo; bar;"),
            Ok(vec![
                Diagnostic::error(0, "near \"foo\": syntax error"),
                Diagnostic::error(5, "near \"bar\": syntax error"),
            ]),
        );
    }

    #[test]
    fn test_ignore_errors_in_pragma_stmt() {
        assert_eq!(diagnose("PRAGMA foo bar"), Ok(vec![]));
    }

    #[test]
    fn test_pragma_functions() {
        assert_eq!(diagnose("SELECT * FROM pragma_index_info(;").unwrap().len(), 1);
    }

    #[test]
    fn test_ignore_errors_in_explain_stmt() {
        assert_eq!(diagnose("EXPLAIN foo bar"), Ok(vec![]));
        assert_eq!(diagnose("EXPLAIN QUERY PLAN foo bar"), Ok(vec![]));
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(diagnose(";;"), Ok(vec![]));
    }

    #[test]
    fn test_unknown_table_option() {
        assert_eq!(
            diagnose("CREATE TABLE t(c) foobar"),
            Ok(vec![Diagnostic::error(24, "unknown table option: foobar")]),
        );
    }
}
