use crate::{
    code_lens::{code_lens, get_table_to_be_focused, CodeLens, CodeLensKind, TableToBeFocused},
    list_placeholders::{Placeholder, PlaceholderRange},
    split_statements::split_sqlite_statements,
    tokenize::ZeroIndexedLocation,
};

#[test]
fn test_select() {
    assert_eq!(
        code_lens("SELECT 1; SELECT 2; VALUES(3);"),
        [
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 9),
                stmt_executed: "SELECT 1;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: None,
            },
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 10),
                end: ZeroIndexedLocation::new(0, 19),
                stmt_executed: "SELECT 2;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: None,
            },
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 20),
                end: ZeroIndexedLocation::new(0, 30),
                stmt_executed: "VALUES(3);".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: None,
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
                start: ZeroIndexedLocation::new(0, 5),
                end: ZeroIndexedLocation::new(0, 6),
                stmt_executed: "WITH a AS (SELECT 1) SELECT * FROM `a`".to_owned(),
                cte_identifier: Some("`a`".to_owned()),
                placeholders: vec![],
                table_to_be_focused: None,
            },
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 30),
                stmt_executed: "WITH a AS (SELECT 1) SELECT 2;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: None,
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
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 13),
                stmt_executed: "DROP TABLE t;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: None,
            },
            CodeLens {
                kind: CodeLensKind::Other,
                start: ZeroIndexedLocation::new(0, 14),
                end: ZeroIndexedLocation::new(0, 31),
                stmt_executed: "ATTACH 'db' as db".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: None,
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
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 31),
            stmt_executed: "BEGIN; SELECT 1; SELECT 2; END;".to_owned(),
            cte_identifier: None,
            placeholders: vec![],
            table_to_be_focused: None,
        }]
    );
}

#[test]
fn test_pragma() {
    assert_eq!(
        code_lens("PRAGMA analysis_limit;"),
        [CodeLens {
            kind: CodeLensKind::Other,
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 22),
            stmt_executed: "PRAGMA analysis_limit;".to_owned(),
            cte_identifier: None,
            placeholders: vec![],
            table_to_be_focused: None,
        }]
    );
}

#[test]
fn test_vacuum() {
    assert_eq!(
        code_lens("VACUUM;"),
        [CodeLens {
            kind: CodeLensKind::Other,
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 7),
            stmt_executed: "VACUUM;".to_owned(),
            cte_identifier: None,
            placeholders: vec![],
            table_to_be_focused: None,
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
                start: ZeroIndexedLocation::new(0, 5),
                end: ZeroIndexedLocation::new(0, 6),
                stmt_executed: "WITH x AS (SELECT 1) SELECT * FROM `x`".to_owned(),
                cte_identifier: Some("`x`".to_owned()),
                placeholders: vec![],
                table_to_be_focused: None,
            },
            CodeLens {
                kind: CodeLensKind::Other,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 40),
                stmt_executed: "WITH x AS (SELECT 1) UPDATE t SET a = 1;".to_owned(),
                cte_identifier: None,
                placeholders: vec![],
                table_to_be_focused: Some(TableToBeFocused {
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
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 13),
            stmt_executed: "SELECT ?, :a;".to_owned(),
            cte_identifier: None,
            placeholders: vec![
                Placeholder {
                    name: None,
                    ranges_relative_to_stmt: vec![PlaceholderRange {
                        start: ZeroIndexedLocation::new(0, 7),
                        end: ZeroIndexedLocation::new(0, 8),
                    }],
                },
                Placeholder {
                    name: Some(":a".to_owned()),
                    ranges_relative_to_stmt: vec![PlaceholderRange {
                        start: ZeroIndexedLocation::new(0, 10),
                        end: ZeroIndexedLocation::new(0, 12),
                    }]
                }
            ],
            table_to_be_focused: None,
        }]
    );
}

#[test]
fn test_placeholders_with_prefix() {
    assert_eq!(
        code_lens("-- comment\n SELECT ?, :a;"),
        [CodeLens {
            kind: CodeLensKind::Select,
            start: ZeroIndexedLocation::new(1, 1),
            end: ZeroIndexedLocation::new(1, 14),
            stmt_executed: "SELECT ?, :a;".to_owned(),
            cte_identifier: None,
            placeholders: vec![
                Placeholder {
                    name: None,
                    ranges_relative_to_stmt: vec![PlaceholderRange {
                        start: ZeroIndexedLocation::new(0, 7),
                        end: ZeroIndexedLocation::new(0, 8),
                    }],
                },
                Placeholder {
                    name: Some(":a".to_owned()),
                    ranges_relative_to_stmt: vec![PlaceholderRange {
                        start: ZeroIndexedLocation::new(0, 10),
                        end: ZeroIndexedLocation::new(0, 12),
                    }]
                }
            ],
            table_to_be_focused: None,
        }]
    );
}

#[test]
fn test_get_table_to_be_focused_create_table() {
    assert_eq!(
        get_table_to_be_focused(&split_sqlite_statements("CREATE TABLE table1").unwrap().0[0].real_tokens),
        Some(TableToBeFocused {
            schema: None,
            table: "table1".to_owned(),
        })
    );
}

#[test]
fn test_get_table_to_be_focused_insert_into() {
    assert_eq!(
        get_table_to_be_focused(&split_sqlite_statements("INSERT INTO table1").unwrap().0[0].real_tokens),
        Some(TableToBeFocused {
            schema: None,
            table: "table1".to_owned(),
        })
    );
}

#[test]
fn test_get_table_to_be_focused_insert_into_2() {
    assert_eq!(
        get_table_to_be_focused(&split_sqlite_statements("INSERT INTO \"table1\"").unwrap().0[0].real_tokens),
        Some(TableToBeFocused {
            schema: None,
            table: "table1".to_owned(),
        })
    );
}

#[test]
fn test_get_table_to_be_focused_insert_into_3() {
    assert_eq!(
        get_table_to_be_focused(&split_sqlite_statements("INSERT INTO \"table 1\"").unwrap().0[0].real_tokens),
        Some(TableToBeFocused {
            schema: None,
            table: "table 1".to_owned(),
        })
    );
}

#[test]
fn test_get_table_to_be_focused_insert_schema() {
    assert_eq!(
        get_table_to_be_focused(&split_sqlite_statements("INSERT INTO schema1.table1").unwrap().0[0].real_tokens),
        Some(TableToBeFocused {
            schema: Some("schema1".to_owned()),
            table: "table1".to_owned(),
        })
    );
}

#[test]
fn test_get_table_to_be_focused_create_table_schema() {
    assert_eq!(
        get_table_to_be_focused(&split_sqlite_statements("CREATE TABLE schema1.table1").unwrap().0[0].real_tokens),
        Some(TableToBeFocused {
            schema: Some("schema1".to_owned()),
            table: "table1".to_owned(),
        })
    );
}

#[test]
fn test_get_table_to_be_focused_create_trigger() {
    assert_eq!(
        get_table_to_be_focused(
            &split_sqlite_statements(
                "CREATE TRIGGER trigger_insert AFTER INSERT ON t INSERT INTO table1 VALUES (1); END"
            )
            .unwrap()
            .0[0]
                .real_tokens
        ),
        None
    );
}
