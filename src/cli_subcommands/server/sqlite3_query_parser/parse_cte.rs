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

    let mut paren_depth: i64 = 0;
    let mut current_entry: Option<CTEEntry> = None;

    // Search forward
    // -------------->
    // WITH name1(x) AS (SELECT ...), name2 AS (SELECT ...) SELECT ...
    for (i, token) in stmt.real_tokens.iter().enumerate() {
        // Update paren_depth
        match &token.value {
            SQLite3Token::LParen => {
                paren_depth += 1;
            }
            SQLite3Token::RParen => {
                paren_depth = (paren_depth - 1).max(0);
            }
            _ => {}
        }

        if let Some(current_entry_inner) = &mut current_entry {
            // Find the subquery
            match &token.value {
                SQLite3Token::LParen if paren_depth == 1 => {
                    current_entry_inner.query_range.start = token.range.end.clone();
                    current_entry_inner.query_range.end = token.range.end.clone(); // placeholder
                    continue;
                }
                SQLite3Token::RParen if paren_depth == 0 => {
                    current_entry_inner.query_range.end = token.range.start.clone();
                    entries.push(current_entry_inner.to_owned());
                    current_entry = None;
                }
                _ => {}
            }
        } else if paren_depth == 0 {
            // Find AS and the end of the WITH clause
            if let SQLite3Token::Keyword(kwd) = &token.value {
                match kwd {
                    SQLite3Keyword::AS => {
                        //      Search backward
                        //      <--------
                        // WITH name1(x) AS (SELECT ...), name2 AS (SELECT ...) SELECT ...

                        // Find the last identifier
                        let mut paren_depth2 = 0;
                        for j in (0..(i.saturating_sub(1))).rev() {
                            match stmt.real_tokens[j].value {
                                SQLite3Token::Whitespace(_) => {}
                                SQLite3Token::LParen => { paren_depth2 -= 1;}
                                SQLite3Token::RParen => { paren_depth2 += 1;}
                                _ if paren_depth2 == 0 => {
                                    if let SQLite3Token::Identifier(word, _) = &stmt.real_tokens[j].value {
                                        current_entry = Some(CTEEntry {
                                            ident: WithZeroIndexedRange {
                                                range: stmt.real_tokens[j].range.clone(),
                                                value: word.clone(),
                                            },
                                            query_range: ZeroIndexedRange::new(token.range.end.clone(), token.range.end.clone()), // placeholder
                                        });
                                    }  // else => syntax error
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }

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
                    | SQLite3Keyword::DROP => {
                        return Some(CommonTableExpression {
                            entries,
                            body_range: ZeroIndexedRange::new(token.range.start.clone(), stmt.real_text.range.end.clone()),
                        })
                    },
                    _ => {}
                }
            }
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
                vec![
                    CTEEntryString::new("ident1", "SELECT 1"),
                    CTEEntryString::new("ident2", "SELECT 2"),
                ],
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
            parse_cte_then_slice_string(
                "WITH ident1 AS MATERIALIZED (SELECT 1), ident2 AS NOT MATERIALIZED (SELECT 2) SELECT 3;"
            ),
            CTEString::new(
                vec![
                    CTEEntryString::new("ident1", "SELECT 1"),
                    CTEEntryString::new("ident2", "SELECT 2"),
                ],
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
