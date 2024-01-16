use serde::{Deserialize, Serialize};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, Word};

use crate::keywords::START_OF_STATEMENT_KEYWORDS_UNSUPPORTED_BY_SQLPARSER;
use crate::parse_cte::parse_cte;
use crate::split_statements::{get_text_range, split_sqlite_statements};
use crate::sqlite3_driver::escape_sql_identifier;
use crate::tokenize::ZeroIndexedLocation;

/// Represents the kind of code lens.
#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[ts(export)]
pub enum CodeLensKind {
    Select,
    Explain,
    Other,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct CodeLens {
    pub kind: CodeLensKind,
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
    pub stmt_executed: String,
}

/// Returns a list of code lenses for the given SQL input.
pub fn code_lens(sql: &str) -> Vec<CodeLens> {
    let mut code_lens: Vec<CodeLens> = vec![];
    let lines = sql.lines().collect::<Vec<_>>();

    // For each statement
    for stmt in split_sqlite_statements(sql).unwrap_or_default() {
        if stmt.real_tokens.is_empty() {
            continue;
        }

        // Parse CTE
        let cte = parse_cte(&stmt);
        let mut cte_end = ZeroIndexedLocation::new(0, 0);
        if let Some(cte) = cte {
            for entry in cte.entries {
                let with_clause = get_text_range(&lines, &stmt.real_start, &cte.body_start);
                let select_stmt = format!(
                    "{}SELECT * FROM {}",
                    if with_clause.ends_with(' ') { "" } else { " " },
                    escape_sql_identifier(&get_text_range(&lines, &entry.ident_start, &entry.ident_end))
                );
                code_lens.push(CodeLens {
                    kind: CodeLensKind::Select,
                    stmt_executed: with_clause + &select_stmt,
                    start: entry.ident_start,
                    end: entry.ident_end,
                })
            }
            cte_end = cte.body_start;
        }

        let mut kind: Option<CodeLensKind> = None;
        for token in stmt.real_tokens {
            if token.start < cte_end {
                continue;
            }
            if let Token::Word(w) = token.token {
                match w {
                    Word {
                        keyword: Keyword::SELECT | Keyword::VALUES,
                        ..
                    } => {
                        kind = Some(CodeLensKind::Select);
                        break;
                    }
                    Word {
                        keyword: Keyword::EXPLAIN,
                        ..
                    } => {
                        kind = Some(CodeLensKind::Explain);
                        break;
                    }
                    Word {
                        keyword:
                            Keyword::INSERT
                            | Keyword::DELETE
                            | Keyword::UPDATE
                            | Keyword::REPLACE
                            | Keyword::MERGE
                            | Keyword::DROP
                            | Keyword::CREATE
                            | Keyword::ALTER
                            | Keyword::PROGRAM
                            | Keyword::ANALYZE
                            | Keyword::BEGIN
                            | Keyword::VACUUM,
                        ..
                    } => {
                        kind = Some(CodeLensKind::Other);
                        break;
                    }
                    // keywords that sqlparser does not support
                    Word {
                        quote_style: None,
                        value,
                        keyword: Keyword::NoKeyword,
                    } if START_OF_STATEMENT_KEYWORDS_UNSUPPORTED_BY_SQLPARSER
                        .contains(value.to_uppercase().as_str()) =>
                    {
                        kind = Some(CodeLensKind::Other);
                        break;
                    }
                    _ => {}
                }
            }
        }

        if let Some(kind) = kind {
            code_lens.push(CodeLens {
                kind,
                start: stmt.real_start,
                end: stmt.real_end,
                stmt_executed: stmt.real_text,
            })
        }
    }

    code_lens
}
