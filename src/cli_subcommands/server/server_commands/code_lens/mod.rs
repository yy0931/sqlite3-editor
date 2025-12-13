mod get_table_to_be_modified;
mod list_placeholders;

use crate::cli_error::CLIError;
use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::server_commands::code_lens::get_table_to_be_modified::get_table_to_be_modified;
use crate::cli_subcommands::server::server_commands::code_lens::get_table_to_be_modified::TableToBeModified;
use crate::cli_subcommands::server::server_commands::code_lens::list_placeholders::list_placeholders;
use crate::cli_subcommands::server::server_commands::code_lens::list_placeholders::Placeholder;
use crate::cli_subcommands::server::sqlite3_query_parser::parse_cte::parse_cte;
use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::split_sqlite_statements;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Keyword;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedLocation;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;
use crate::cli_subcommands::server::TruncateAll;
use crate::sqlite_escape::escape_sql_identifier;
use rmp_serde::encode::write_named;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Write;

// Handle the command
pub fn run(r: &mut File, w: &mut File) -> CLIErrorCode {
    if let Err(err) = rmp_serde::from_read(r).map(
        |CodeLensCommandParams { query }: CodeLensCommandParams| -> std::result::Result<(), CLIError> {
            write_named(w, &code_lens(&query))?;
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

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(from = "(String,)")]
pub struct CodeLensCommandParams {
    pub query: String,
}

impl From<(String,)> for CodeLensCommandParams {
    fn from(value: (String,)) -> Self {
        Self { query: value.0 }
    }
}

/// Returns a list of code lenses for the given SQL input.
pub fn code_lens(sql: &str) -> Vec<CodeLens> {
    let mut code_lens: Vec<CodeLens> = vec![];
    let lines = sql.lines().collect::<Vec<_>>();

    // For each statement
    for stmt in split_sqlite_statements(sql).unwrap_or_default().0 {
        if stmt.real_tokens.is_empty() {
            continue;
        }

        // Parse CTE
        let cte = parse_cte(&stmt);
        let mut cte_end = ZeroIndexedLocation::new(0, 0);
        if let Some(cte) = cte {
            for entry in cte.entries {
                let with_clause = ZeroIndexedRange::new(stmt.real_text.range.start.clone(), cte.body_range.start.clone()).get_text(&lines);
                let cte_ident = escape_sql_identifier(&entry.ident.range.get_text(&lines));
                let select_stmt = format!(
                    "{}SELECT * FROM {}",
                    if with_clause.ends_with(' ') { "" } else { " " },
                    cte_ident.clone()
                );
                let stmt_executed = with_clause + &select_stmt;
                let Ok((cte_stmt, _)) = split_sqlite_statements(&stmt_executed) else {
                    continue;
                };
                let Some(cte_stmt) = cte_stmt.first() else {
                    continue;
                };
                code_lens.push(CodeLens {
                    kind: CodeLensKind::Select,
                    placeholders: list_placeholders(cte_stmt),
                    stmt_executed,
                    range: entry.ident.range,
                    cte_identifier: Some(cte_ident),
                    table_to_be_shown: None, // is None because cte_stmt is always a SELECT statement
                })
            }
            cte_end = cte.body_range.start;
        }

        let mut kind: Option<CodeLensKind> = None;
        let placeholders = list_placeholders(&stmt);
        for token in &stmt.real_tokens {
            if token.range.start >= cte_end {
                if let SQLite3Token::Keyword(k) = token.value {
                    match k {
                        SQLite3Keyword::SELECT | SQLite3Keyword::VALUES => {
                            kind = Some(CodeLensKind::Select);
                            break;
                        }
                        SQLite3Keyword::EXPLAIN => {
                            kind = Some(CodeLensKind::Explain);
                            break;
                        }
                        SQLite3Keyword::INSERT
                        | SQLite3Keyword::DELETE
                        | SQLite3Keyword::UPDATE
                        | SQLite3Keyword::REPLACE
                        | SQLite3Keyword::DROP
                        | SQLite3Keyword::CREATE
                        | SQLite3Keyword::ALTER
                        | SQLite3Keyword::ANALYZE
                        | SQLite3Keyword::BEGIN
                        | SQLite3Keyword::VACUUM
                        | SQLite3Keyword::ATTACH
                        | SQLite3Keyword::DETACH
                        | SQLite3Keyword::PRAGMA
                        | SQLite3Keyword::REINDEX => {
                            kind = Some(CodeLensKind::Other);
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        if let Some(kind) = kind {
            code_lens.push(CodeLens {
                kind,
                range: stmt.real_text.range,
                stmt_executed: stmt.real_text.value,
                cte_identifier: None,
                placeholders,
                table_to_be_shown: get_table_to_be_modified(&stmt.real_tokens),
            })
        }
    }

    code_lens
}

/// Represents the kind of code lens.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub enum CodeLensKind {
    Select,
    Explain,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct CodeLens {
    pub kind: CodeLensKind,
    pub range: ZeroIndexedRange,
    pub stmt_executed: String,
    pub cte_identifier: Option<String>,
    pub placeholders: Vec<Placeholder>,
    pub table_to_be_shown: Option<TableToBeModified>,
}

#[cfg(test)]
mod test {
    use crate::cli_subcommands::server::server_commands::code_lens::code_lens;
    use crate::cli_subcommands::server::server_commands::code_lens::get_table_to_be_modified::TableToBeModified;
    use crate::cli_subcommands::server::server_commands::code_lens::list_placeholders::Placeholder;
    use crate::cli_subcommands::server::server_commands::code_lens::CodeLens;
    use crate::cli_subcommands::server::server_commands::code_lens::CodeLensKind;
    use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;

    #[test]
    fn test_select() {
        assert_eq!(
            code_lens("SELECT 1; SELECT 2; VALUES(3);"),
            [
                CodeLens {
                    kind: CodeLensKind::Select,
                    range: ZeroIndexedRange::new4(0, 0, 0, 9),
                    stmt_executed: "SELECT 1;".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
                CodeLens {
                    kind: CodeLensKind::Select,
                    range: ZeroIndexedRange::new4(0, 10, 0, 19),
                    stmt_executed: "SELECT 2;".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
                CodeLens {
                    kind: CodeLensKind::Select,
                    range: ZeroIndexedRange::new4(0, 20, 0, 30),
                    stmt_executed: "VALUES(3);".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
            ]
        );
    }

    #[test]
    fn test_with_clause() {
        assert_eq!(
            code_lens("WITH a AS (SELECT 1) SELECT 2;"),
            [
                CodeLens {
                    kind: CodeLensKind::Select,
                    range: ZeroIndexedRange::new4(0, 5, 0, 6),
                    stmt_executed: "WITH a AS (SELECT 1) SELECT * FROM `a`".to_owned(),
                    cte_identifier: Some("`a`".to_owned()),
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
                CodeLens {
                    kind: CodeLensKind::Select,
                    range: ZeroIndexedRange::new4(0, 0, 0, 30),
                    stmt_executed: "WITH a AS (SELECT 1) SELECT 2;".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
            ]
        );
    }

    #[test]
    fn test_explain() {
        assert_eq!(code_lens("EXPLAIN SELECT 1;")[0].kind, CodeLensKind::Explain);
    }

    #[test]
    fn test_other() {
        assert_eq!(
            code_lens("DROP TABLE t; ATTACH 'db' as db"),
            [
                CodeLens {
                    kind: CodeLensKind::Other,
                    range: ZeroIndexedRange::new4(0, 0, 0, 13),
                    stmt_executed: "DROP TABLE t;".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
                CodeLens {
                    kind: CodeLensKind::Other,
                    range: ZeroIndexedRange::new4(0, 14, 0, 31),
                    stmt_executed: "ATTACH 'db' as db".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: None,
                }
            ]
        );
    }

    #[test]
    fn test_begin_end() {
        assert_eq!(
            code_lens("BEGIN; SELECT 1; SELECT 2; END;"),
            [CodeLens {
                kind: CodeLensKind::Other,
                range: ZeroIndexedRange::new4(0, 0, 0, 31),
                stmt_executed: "BEGIN; SELECT 1; SELECT 2; END;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_shown: None,
            }]
        );
    }

    #[test]
    fn test_pragma() {
        assert_eq!(
            code_lens("PRAGMA analysis_limit;"),
            [CodeLens {
                kind: CodeLensKind::Other,
                range: ZeroIndexedRange::new4(0, 0, 0, 22),
                stmt_executed: "PRAGMA analysis_limit;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_shown: None,
            }]
        );
    }

    #[test]
    fn test_vacuum() {
        assert_eq!(
            code_lens("VACUUM;"),
            [CodeLens {
                kind: CodeLensKind::Other,
                range: ZeroIndexedRange::new4(0, 0, 0, 7),
                stmt_executed: "VACUUM;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_shown: None,
            }]
        );
    }

    #[test]
    fn test_with_update() {
        assert_eq!(
            code_lens("WITH x AS (SELECT 1) UPDATE t SET a = 1;"),
            [
                CodeLens {
                    kind: CodeLensKind::Select,
                    range: ZeroIndexedRange::new4(0, 5, 0, 6),
                    stmt_executed: "WITH x AS (SELECT 1) SELECT * FROM `x`".to_owned(),
                    cte_identifier: Some("`x`".to_owned()),
                    placeholders: vec![],
                    table_to_be_shown: None,
                },
                CodeLens {
                    kind: CodeLensKind::Other,
                    range: ZeroIndexedRange::new4(0, 0, 0, 40),
                    stmt_executed: "WITH x AS (SELECT 1) UPDATE t SET a = 1;".to_owned(),
                    cte_identifier: None,
                    placeholders: vec![],
                    table_to_be_shown: Some(TableToBeModified {
                        schema: None,
                        table: "t".to_owned()
                    }),
                }
            ]
        );
    }

    #[test]
    fn test_placeholders() {
        assert_eq!(
            code_lens("SELECT ?, :a;"),
            [CodeLens {
                kind: CodeLensKind::Select,
                range: ZeroIndexedRange::new4(0, 0, 0, 13),
                stmt_executed: "SELECT ?, :a;".to_owned(),
                cte_identifier: None,
                placeholders: vec![
                    Placeholder::new(None, vec![ZeroIndexedRange::new4(0, 7, 0, 8)]),
                    Placeholder::new(Some(":a".to_owned()), vec![ZeroIndexedRange::new4(0, 10, 0, 12)])
                ],
                table_to_be_shown: None,
            }]
        );
    }

    #[test]
    fn test_placeholders_with_prefix() {
        assert_eq!(
            code_lens("-- comment\n SELECT ?, :a;"),
            [CodeLens {
                kind: CodeLensKind::Select,
                range: ZeroIndexedRange::new4(1, 1, 1, 14),
                stmt_executed: "SELECT ?, :a;".to_owned(),
                cte_identifier: None,
                placeholders: vec![
                    Placeholder::new(None, vec![ZeroIndexedRange::new4(0, 7, 0, 8)]),
                    Placeholder::new(Some(":a".to_owned()), vec![ZeroIndexedRange::new4(0, 10, 0, 12)])
                ],
                table_to_be_shown: None,
            }]
        );
    }
}
