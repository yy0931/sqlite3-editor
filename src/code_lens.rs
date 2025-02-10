use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, Word};

use crate::keywords::{KEYWORDS_UNSUPPORTED_BY_SQLPARSER, START_OF_STATEMENT_KEYWORDS_UNSUPPORTED_BY_SQLPARSER};
use crate::list_placeholders::{list_placeholders, Placeholder};
use crate::parse_cte::parse_cte;
use crate::split_statements::{get_text_range, split_sqlite_statements};
use crate::sqlite3::escape_sql_identifier;
use crate::tokenize::{TokenWithRangeLocation, ZeroIndexedLocation};

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
pub struct TableToBeFocused {
    pub schema: Option<String>,
    pub table: String,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[ts(export)]
pub struct CodeLens {
    pub kind: CodeLensKind,
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
    pub stmt_executed: String,
    pub cte_identifier: Option<String>,
    pub placeholders: Vec<Placeholder>,
    pub table_to_be_focused: Option<TableToBeFocused>,
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
                let with_clause = get_text_range(&lines, &stmt.real_start, &cte.body_start);
                let cte_ident = escape_sql_identifier(&get_text_range(&lines, &entry.ident_start, &entry.ident_end));
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
                    start: entry.ident_start,
                    end: entry.ident_end,
                    cte_identifier: Some(cte_ident),
                    table_to_be_focused: None, // is None because cte_stmt is always a SELECT statement
                })
            }
            cte_end = cte.body_start;
        }

        let mut kind: Option<CodeLensKind> = None;
        let placeholders = list_placeholders(&stmt);
        for token in &stmt.real_tokens {
            if token.start < cte_end {
                continue;
            }
            if let Token::Word(w) = &token.token {
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
                cte_identifier: None,
                placeholders,
                table_to_be_focused: get_table_to_be_focused(&stmt.real_tokens),
            })
        }
    }

    code_lens
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PatternItem {
    Keyword(Keyword),
    WordAsSchema,
    Period,
    WordAsTable,
}

static PATTERN_MAP: Lazy<HashMap<Keyword, Vec<Vec<PatternItem>>>> = Lazy::new(|| {
    fn k(k: Keyword) -> PatternItem {
        PatternItem::Keyword(k)
    }

    let mut map = HashMap::<Keyword, Vec<Vec<PatternItem>>>::new();

    trait AddPattern {
        fn add_pattern(&mut self, first_token: Keyword, pattern: &[PatternItem], follows_table_name: bool);
    }

    impl AddPattern for HashMap<Keyword, Vec<Vec<PatternItem>>> {
        fn add_pattern(&mut self, first_token: Keyword, pattern: &[PatternItem], follows_table_name: bool) {
            if follows_table_name {
                {
                    let mut vec = pattern.to_vec();
                    vec.extend([PatternItem::WordAsSchema, PatternItem::Period, PatternItem::WordAsTable]);
                    self.entry(first_token).or_default().push(vec);
                }
                {
                    let mut vec = pattern.to_vec();
                    vec.extend([PatternItem::WordAsTable]);
                    self.entry(first_token).or_default().push(vec);
                }
            } else {
                self.entry(first_token).or_default().push(pattern.to_vec());
            }
        }
    }

    {
        use Keyword::*;
        map.add_pattern(CREATE, &[k(TRIGGER)], false); // CREATE TRIGGER ... INSERT INTO table1 ... should not show table1
        map.add_pattern(CREATE, &[k(TEMP), k(TRIGGER)], false);
        map.add_pattern(CREATE, &[k(TEMPORARY), k(TRIGGER)], false);
        map.add_pattern(CREATE, &[k(TABLE)], true);
        map.add_pattern(CREATE, &[k(TABLE), k(IF), k(NOT), k(EXISTS)], true);
        map.add_pattern(CREATE, &[k(VIEW)], true);
        map.add_pattern(CREATE, &[k(VIEW), k(IF), k(NOT), k(EXISTS)], true);
        map.add_pattern(CREATE, &[k(VIRTUAL), k(TABLE)], true);
        map.add_pattern(CREATE, &[k(VIRTUAL), k(TABLE), k(IF), k(NOT), k(EXISTS)], true);
        map.add_pattern(REPLACE, &[k(INTO)], true);
        map.add_pattern(INSERT, &[k(INTO)], true);
        map.add_pattern(INSERT, &[k(OR), k(ABORT), k(INTO)], true);
        map.add_pattern(INSERT, &[k(OR), k(FAIL), k(INTO)], true);
        map.add_pattern(INSERT, &[k(OR), k(IGNORE), k(INTO)], true);
        map.add_pattern(INSERT, &[k(OR), k(REPLACE), k(INTO)], true);
        map.add_pattern(INSERT, &[k(OR), k(ROLLBACK), k(INTO)], true);
        map.add_pattern(UPDATE, &[], true);
        map.add_pattern(UPDATE, &[k(OR), k(ABORT), k(INTO)], true);
        map.add_pattern(UPDATE, &[k(OR), k(FAIL), k(INTO)], true);
        map.add_pattern(UPDATE, &[k(OR), k(IGNORE), k(INTO)], true);
        map.add_pattern(UPDATE, &[k(OR), k(REPLACE), k(INTO)], true);
        map.add_pattern(UPDATE, &[k(OR), k(ROLLBACK), k(INTO)], true);
        map.add_pattern(ALTER, &[k(TABLE)], true);
    }

    map
});

fn match_pattern(tokens: &[&TokenWithRangeLocation], pattern: &[PatternItem]) -> Option<Option<TableToBeFocused>> {
    let mut schema: Option<String> = None;
    let mut table: Option<String> = None;

    // Make sure we have enough tokens.
    if pattern.len() > tokens.len() {
        return None;
    }

    for (j, pattern_item) in pattern.iter().enumerate() {
        let token = &tokens[j].token;
        match pattern_item {
            PatternItem::Keyword(expected_keyword) => {
                match token {
                    Token::Word(Word {
                        quote_style: None,
                        keyword,
                        ..
                    }) if keyword == expected_keyword => { /* continue */ }
                    _ => return None,
                }
            }
            PatternItem::Period => {
                if *token != Token::Period {
                    return None;
                }
            }
            PatternItem::WordAsSchema | PatternItem::WordAsTable => match token {
                Token::Word(Word {
                    keyword: Keyword::NoKeyword | Keyword::ENCODING,
                    value,
                    ..
                }) if !KEYWORDS_UNSUPPORTED_BY_SQLPARSER.contains(&value.to_uppercase().as_str()) => {
                    if *pattern_item == PatternItem::WordAsSchema {
                        schema = Some(value.clone());
                    } else {
                        table = Some(value.clone());
                    }
                }
                _ => return None,
            },
        }
    }

    // Returns Some(None) if the pattern is CREATE TRIGGER.
    // Returns Some(Some(...)) otherwise.
    Some(table.map(|table| TableToBeFocused { schema, table }))
}

pub fn get_table_to_be_focused(real_tokens: &[TokenWithRangeLocation]) -> Option<TableToBeFocused> {
    // Filter out whitespace tokens.
    let filtered: Vec<&TokenWithRangeLocation> = real_tokens
        .iter()
        .filter(|t| !matches!(t.token, Token::Whitespace(_)))
        .collect();

    for (i, token) in filtered.iter().enumerate() {
        // We only start matching if a token is a Word with no quoting.
        let Token::Word(Word {
            quote_style: None,
            keyword,
            ..
        }) = &token.token
        else {
            continue;
        };

        let Some(patterns) = PATTERN_MAP.get(keyword) else {
            continue;
        };

        for pattern in patterns {
            if i + pattern.len() < filtered.len() {
                if let Some(result) = match_pattern(&filtered[i + 1..], pattern) {
                    return result;
                }
            }
        }
    }
    None
}
