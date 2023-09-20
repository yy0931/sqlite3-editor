use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sqlparser::tokenizer::{Location, TokenizerError};

use crate::{
    split_statements::{split_sqlite_statements, SplittedStatement},
    tokenize::ZeroIndexedLocation,
};

lazy_static! {
    static ref PRAGMA: regex::Regex = regex::Regex::new(r#"(?i)(?s).*\bPRAGMA[^_a-zA-Z0-9]"#).unwrap();
    static ref QUERY: regex::Regex = regex::Regex::new(r#"(?i)(?s).*\bQUERY[^_a-zA-Z0-9]"#).unwrap();
    static ref EXPLAIN: regex::Regex = regex::Regex::new(r#"(?i)(?s).*\bEXPLAIN[^_a-zA-Z0-9]"#).unwrap();
}

lazy_static! {
    static ref SQL_INPUT_ERROR_SYNTAX_ERROR: regex::Regex =
        regex::Regex::new(r#"(?i)(?s)^(?:near .*: syntax error|unrecognized token:|incomplete input)"#).unwrap();
    static ref SQLITE_FAILURE_SYNTAX_ERROR: regex::Regex =
        regex::Regex::new(r#"(?i)(?s)^(?:unknown table option)"#).unwrap();
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct PossibleCause {
    pub offset: usize,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub possible_causes: Vec<PossibleCause>,
    pub message: String,
    pub severity: Severity,
}

/// Checks the syntax of a single SQL statement using a SQLite connection.
fn check_syntax_stmt(stmt_str: &str, conn: &mut rusqlite::Connection, offset_start: usize) -> Option<Diagnostic> {
    if stmt_str.trim() == "" || stmt_str.trim() == ";" {
        return None;
    }

    if !PRAGMA.is_match(&stmt_str) && !QUERY.is_match(&stmt_str) && !EXPLAIN.is_match(&stmt_str) {
        let explain = format!("EXPLAIN {stmt_str}");
        let prepare = conn.prepare(&explain);
        match prepare {
            Err(rusqlite::Error::SqlInputError { sql, offset, msg, .. })
                if SQL_INPUT_ERROR_SYNTAX_ERROR.is_match(&msg) =>
            {
                return Some(Diagnostic {
                    possible_causes: vec![PossibleCause {
                        offset: offset_start + loose_byte_to_code_point_index(&sql, offset as usize) - "EXPLAIN ".len(),
                    }],
                    severity: Severity::Error,
                    message: msg,
                });
            }
            Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if SQLITE_FAILURE_SYNTAX_ERROR.is_match(&msg) => {
                return Some(Diagnostic {
                    possible_causes: vec![PossibleCause {
                        offset: offset_start + stmt_str.chars().count(),
                    }],
                    message: msg,
                    severity: Severity::Error,
                });
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Checks the syntax of a string containing SQL statements.
pub fn check_syntax(sql: &str) -> rusqlite::Result<Vec<Diagnostic>> {
    let mut errors: Vec<Diagnostic> = vec![];
    let statements = match split_sqlite_statements(sql) {
        Ok(statements) => statements,
        Err(TokenizerError { line, col, message }) => {
            errors.push(Diagnostic {
                possible_causes: vec![PossibleCause {
                    offset: Into::<ZeroIndexedLocation>::into(Location { line, column: col }).offset_at(sql),
                }],
                message,
                severity: Severity::Error,
            });
            return Ok(errors);
        }
    };
    let mut conn = rusqlite::Connection::open_in_memory()?;
    for SplittedStatement {
        real_text: stmt_str,
        real_start,
        ..
    } in statements
    {
        if let Some(err) = check_syntax_stmt(&stmt_str, &mut conn, real_start.offset_at(&sql)) {
            errors.push(err);
        }
    }
    Ok(errors)
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
