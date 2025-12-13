use crate::cli_subcommands::server::sqlite3_query_parser::paren_aware_token_scanner::SQLite3ParenAwareTokenScanner;
use crate::cli_subcommands::server::sqlite3_query_parser::paren_aware_token_scanner::SQLite3TokenScannerOutput;
use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::SingleStatement;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Keyword;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::WithZeroIndexedRange;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;

/// Parses the CTE in the given SQL statement.
/// Returns None if the statement does not have a CTE.
pub fn parse_cte(stmt: &SingleStatement) -> Option<CommonTableExpression> {
    // Return None if the first token is not "WITH"
    if !stmt
        .real_tokens
        .first()
        .is_some_and(|t| t.value == SQLite3Token::Keyword(SQLite3Keyword::WITH))
    {
        return None;
    }

    let mut entries = Vec::<CTEEntry>::new();
    let mut current_entry: Option<CTEEntry> = None;

    // Search forward
    // -------------->
    // WITH name1(x) AS (SELECT ...), name2 AS (SELECT ...) SELECT ...
    let grouped_tokens = SQLite3ParenAwareTokenScanner::new(&stmt.real_tokens).collect::<Vec<_>>();
    for (i, output) in grouped_tokens.iter().enumerate() {
        match output {
            SQLite3TokenScannerOutput::StartOfInput => {}
            SQLite3TokenScannerOutput::EndOfInput => {}

            // subquery
            SQLite3TokenScannerOutput::Group(tokens) if current_entry.is_some() => {
                if let (Some(first_token), Some(last_token)) = (tokens.first(), tokens.last()) {
                    let inner = &mut current_entry.unwrap();
                    inner.query_range.start = first_token.range.start.clone();
                    inner.query_range.end = last_token.range.end.clone();
                    entries.push(inner.to_owned());
                }
                current_entry = None;
            }

            // AS
            SQLite3TokenScannerOutput::SingleToken(WithZeroIndexedRange {
                value: SQLite3Token::Keyword(SQLite3Keyword::AS),
                range,
            }) => {
                //      Search backward
                //      <--------
                // WITH name1(x) AS (SELECT ...), name2 AS (SELECT ...) SELECT ...

                // Find the last identifier
                for grouped_token in grouped_tokens[0..(i.saturating_sub(1))].iter().rev() {
                    if let SQLite3TokenScannerOutput::SingleToken(token) = grouped_token {
                        if let SQLite3Token::Identifier(word, _) = &token.value {
                            current_entry = Some(CTEEntry {
                                ident: WithZeroIndexedRange {
                                    range: token.range.clone(),
                                    value: word.clone(),
                                },
                                query_range: ZeroIndexedRange::new(range.end.clone(), range.end.clone()), // placeholder
                            });
                        } // else => syntax error
                        break;
                    }
                }
            }

            // the end of a WITH clause
            SQLite3TokenScannerOutput::SingleToken(WithZeroIndexedRange {
                value: SQLite3Token::Keyword(
                    // > All common table expressions (ordinary and recursive) are created by prepending a WITH clause in front of a SELECT, INSERT, DELETE, or UPDATE statement.
                    // https://www.sqlite.org/lang_with.html
                    | SQLite3Keyword::SELECT
                    | SQLite3Keyword::INSERT
                    | SQLite3Keyword::DELETE
                    | SQLite3Keyword::UPDATE
                    | SQLite3Keyword::REPLACE
                    | SQLite3Keyword::VALUES

                    // in case
                    | SQLite3Keyword::CREATE
                    | SQLite3Keyword::ALTER
                    | SQLite3Keyword::DROP
                ),
                range,
            }) => {
                return Some(CommonTableExpression {
                    entries,
                    body_range: ZeroIndexedRange::new(range.start.clone(), stmt.real_text.range.end.clone()),
                })
            }

            _ => {}
        }
    }

    None
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CTEEntry {
    pub ident: WithZeroIndexedRange<String>,
    pub query_range: ZeroIndexedRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonTableExpression {
    pub entries: Vec<CTEEntry>,
    pub body_range: ZeroIndexedRange,
}

#[cfg(test)]
mod test {
    use super::super::split_statements::split_sqlite_statements;
    use super::parse_cte;

    #[derive(Debug, Eq, PartialEq, derive_new::new)]
    struct CTEEntryString {
        #[new(into)]
        ident: String,
        #[new(into)]
        query: String,
    }

    #[derive(Debug, Eq, PartialEq, derive_new::new)]
    struct CTEString {
        entries: Vec<CTEEntryString>,
        #[new(into)]
        body: String,
    }

    fn parse_cte_then_slice_string(sql: &str) -> CTEString {
        let cte = parse_cte(&split_sqlite_statements(sql).unwrap().0[0]).unwrap();
        let lines = sql.lines().collect::<Vec<_>>();

        // Convert positions into strings
        CTEString {
            entries: cte
                .entries
                .iter()
                .map(|e| CTEEntryString {
                    ident: e.ident.range.get_text(&lines),
                    query: e.query_range.get_text(&lines),
                })
                .collect::<Vec<_>>(),
            body: cte.body_range.get_text(&lines),
        }
    }

    #[test]
    fn test_simple() {
        // Test the simple case
        assert_eq!(
            parse_cte_then_slice_string("WITH ident1 AS (SELECT 1), ident2 AS (SELECT 2) SELECT 3;"),
            CTEString::new(
                vec![CTEEntryString::new("ident1", "SELECT 1"), CTEEntryString::new("ident2", "SELECT 2"),],
                "SELECT 3;"
            ),
        );
    }

    #[test]
    fn test_nested_paren() {
        // Test nested parentheses
        assert_eq!(
            parse_cte_then_slice_string("WITH ident1 AS (SELECT fn() AS a), ident2 AS (SELECT 2) SELECT 3;"),
            CTEString::new(
                vec![
                    CTEEntryString::new("ident1", "SELECT fn() AS a"),
                    CTEEntryString::new("ident2", "SELECT 2"),
                ],
                "SELECT 3;"
            ),
        );
    }

    #[test]
    fn test_materialized() {
        // Test "MATERIALIZED" and "NOT MATERIALIZED"
        assert_eq!(
            parse_cte_then_slice_string("WITH ident1 AS MATERIALIZED (SELECT 1), ident2 AS NOT MATERIALIZED (SELECT 2) SELECT 3;"),
            CTEString::new(
                vec![CTEEntryString::new("ident1", "SELECT 1"), CTEEntryString::new("ident2", "SELECT 2"),],
                "SELECT 3;"
            ),
        );
    }

    #[test]
    fn test_recursive() {
        // Test "WITH RECURSIVE"
        assert_eq!(
            parse_cte_then_slice_string(
                "
WITH RECURSIVE
  cnt(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM cnt WHERE x<1000000)
SELECT x FROM cnt;"
            ),
            CTEString::new(
                vec![CTEEntryString::new(
                    "cnt",
                    "VALUES(1) UNION ALL SELECT x+1 FROM cnt WHERE x<1000000"
                ),],
                "SELECT x FROM cnt;"
            ),
        );
    }

    #[test]
    fn test_values() {
        assert_eq!(
            parse_cte_then_slice_string("WITH x AS (VALUES(1)) VALUES(2);"),
            CTEString::new(vec![CTEEntryString::new("x", "VALUES(1)"),], "VALUES(2);"),
        );
    }

    #[test]
    fn test_non_cte() {
        assert_eq!(parse_cte(&split_sqlite_statements(";").unwrap().0[0]), None);
        assert_eq!(parse_cte(&split_sqlite_statements("SELECT 1").unwrap().0[0]), None);
    }
}
