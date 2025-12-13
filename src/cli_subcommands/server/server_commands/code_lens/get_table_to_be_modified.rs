use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;

use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Keyword;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::WithZeroIndexedRange;

pub fn get_table_to_be_modified(real_tokens: &[WithZeroIndexedRange<SQLite3Token>]) -> Option<TableToBeModified> {
    // Filter out whitespace tokens.
    let non_whitespace_tokens: Vec<&WithZeroIndexedRange<SQLite3Token>> = real_tokens.iter().filter(|t| !t.value.is_whitespace()).collect();

    for (i, token) in non_whitespace_tokens.iter().enumerate() {
        if let SQLite3Token::Keyword(k) = &token.value {
            if let Some(patterns) = PATTERN_MAP.get(k) {
                for pattern in patterns {
                    if i + pattern.len() < non_whitespace_tokens.len() {
                        if let Some(result) = match_pattern(&non_whitespace_tokens[i + 1..], pattern) {
                            return result;
                        }
                    }
                }
            };
        }
    }
    None
}

static PATTERN_MAP: Lazy<HashMap<SQLite3Keyword, Vec<Vec<PatternItem>>>> = Lazy::new(|| {
    fn k(k: SQLite3Keyword) -> PatternItem {
        PatternItem::Keyword(k)
    }

    let mut map = HashMap::<SQLite3Keyword, Vec<Vec<PatternItem>>>::new();

    trait AddPattern {
        fn add_pattern(&mut self, first_token: SQLite3Keyword, pattern: &[PatternItem], follows_table_name: bool);
    }

    impl AddPattern for HashMap<SQLite3Keyword, Vec<Vec<PatternItem>>> {
        fn add_pattern(&mut self, first_token: SQLite3Keyword, pattern: &[PatternItem], follows_table_name: bool) {
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
        use SQLite3Keyword::*;
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

fn match_pattern(tokens: &[&WithZeroIndexedRange<SQLite3Token>], pattern: &[PatternItem]) -> Option<Option<TableToBeModified>> {
    let mut schema: Option<String> = None;
    let mut table: Option<String> = None;

    // Make sure we have enough tokens.
    if pattern.len() > tokens.len() {
        return None;
    }

    for (j, pattern_item) in pattern.iter().enumerate() {
        let token = &tokens[j].value;
        match pattern_item {
            PatternItem::Keyword(expected_keyword) => {
                if *token != SQLite3Token::Keyword(expected_keyword.to_owned()) {
                    return None;
                }
            }
            PatternItem::Period => {
                if *token != SQLite3Token::Period {
                    return None;
                }
            }
            PatternItem::WordAsSchema | PatternItem::WordAsTable => {
                if let SQLite3Token::Identifier(s, _) = token {
                    if *pattern_item == PatternItem::WordAsSchema {
                        schema = Some(s.clone());
                    } else {
                        table = Some(s.clone());
                    }
                } else {
                    return None;
                }
            }
        }
    }

    // Returns Some(None) if the pattern is CREATE TRIGGER.
    // Returns Some(Some(...)) otherwise.
    Some(table.map(|table| TableToBeModified { schema, table }))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PatternItem {
    Keyword(SQLite3Keyword),
    WordAsSchema,
    Period,
    WordAsTable,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableToBeModified {
    pub schema: Option<String>,
    pub table: String,
}

#[cfg(test)]
mod test {
    use super::get_table_to_be_modified;
    use super::TableToBeModified;
    use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::split_sqlite_statements;

    #[test]
    fn test_get_table_to_be_modified_create_table() {
        assert_eq!(
            get_table_to_be_modified(&split_sqlite_statements("CREATE TABLE table1").unwrap().0[0].real_tokens),
            Some(TableToBeModified {
                schema: None,
                table: "table1".to_owned(),
            })
        );
    }

    #[test]
    fn test_get_table_to_be_modified_insert_into() {
        assert_eq!(
            get_table_to_be_modified(&split_sqlite_statements("INSERT INTO table1").unwrap().0[0].real_tokens),
            Some(TableToBeModified {
                schema: None,
                table: "table1".to_owned(),
            })
        );
    }

    #[test]
    fn test_get_table_to_be_modified_insert_into_2() {
        assert_eq!(
            get_table_to_be_modified(&split_sqlite_statements("INSERT INTO \"table1\"").unwrap().0[0].real_tokens),
            Some(TableToBeModified {
                schema: None,
                table: "table1".to_owned(),
            })
        );
    }

    #[test]
    fn test_get_table_to_be_modified_insert_into_3() {
        assert_eq!(
            get_table_to_be_modified(&split_sqlite_statements("INSERT INTO \"table 1\"").unwrap().0[0].real_tokens),
            Some(TableToBeModified {
                schema: None,
                table: "table 1".to_owned(),
            })
        );
    }

    #[test]
    fn test_get_table_to_be_modified_insert_schema() {
        assert_eq!(
            get_table_to_be_modified(&split_sqlite_statements("INSERT INTO schema1.table1").unwrap().0[0].real_tokens),
            Some(TableToBeModified {
                schema: Some("schema1".to_owned()),
                table: "table1".to_owned(),
            })
        );
    }

    #[test]
    fn test_get_table_to_be_modified_create_table_schema() {
        assert_eq!(
            get_table_to_be_modified(&split_sqlite_statements("CREATE TABLE schema1.table1").unwrap().0[0].real_tokens),
            Some(TableToBeModified {
                schema: Some("schema1".to_owned()),
                table: "table1".to_owned(),
            })
        );
    }

    #[test]
    fn test_get_table_to_be_modified_create_trigger() {
        assert_eq!(
            get_table_to_be_modified(
                &split_sqlite_statements("CREATE TRIGGER trigger_insert AFTER INSERT ON t INSERT INTO table1 VALUES (1); END")
                    .unwrap()
                    .0[0]
                    .real_tokens
            ),
            None
        );
    }
}
