use once_cell::sync::Lazy;

use crate::cli_error::CLIError;

static NON_READONLY_SQL_PATTERN: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)^\s*(INSERT|DELETE|UPDATE|CREATE|DROP|ALTER\s+TABLE)\b").unwrap());

pub fn assert_readonly_query(query: &str, pre_stmt: &Option<String>) -> std::result::Result<(), CLIError> {
    if NON_READONLY_SQL_PATTERN.is_match(query) {
        return CLIError::new_other_error(
            "This query is not allowed in the read-only mode.",
            Some(query.to_owned()),
            None,
        );
    }

    if let Some(pre_stmt) = pre_stmt {
        return CLIError::new_other_error(
            "pre_stmt is not allowed in the read-only mode.",
            Some(pre_stmt.to_owned()),
            None,
        );
    }

    Ok(())
}
