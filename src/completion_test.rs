use std::collections::{HashSet, VecDeque};

use crate::{
    completion::{complete, ColumnCompletion, Completions, TableCompletion, TokenType},
    sqlite3_driver::{ExecMode, SQLite3Driver, TableType},
    tokenize::ZeroIndexedLocation,
};

fn hash_set(values: &[&str]) -> HashSet<String> {
    values.into_iter().map(|v| v.to_string()).collect()
}

fn loc(line: usize, column: usize) -> ZeroIndexedLocation {
    ZeroIndexedLocation { line, column }
}

fn setup() -> SQLite3Driver {
    let mut conn = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    conn.execute(
        "CREATE TABLE table_name(column_name)",
        &[],
        ExecMode::ReadWrite,
        &mut vec![],
    )
    .unwrap();
    conn
}

#[test]
fn test_simple() {
    assert_eq!(
        complete(
            &setup(),
            "WITH cte_ident AS (SELECT 1 as as_ident1) SELECT 1 as as_ident2 FROM table_name",
            &loc(0, 10)
        ),
        Completions {
            table_names: HashSet::from([
                TableCompletion {
                    schema: "main".to_owned(),
                    table: "sqlite_schema".to_owned(),
                    type_: TableType::Table,
                },
                TableCompletion {
                    schema: "temp".to_owned(),
                    table: "sqlite_temp_schema".to_owned(),
                    type_: TableType::Table,
                },
                TableCompletion {
                    schema: "main".to_owned(),
                    table: "table_name".to_owned(),
                    type_: TableType::Table,
                },
            ]),
            schema_names: hash_set(&["main", "temp"]),
            columns_in_tables_that_are_referenced_in_source: HashSet::from([ColumnCompletion {
                schema: "main".to_owned(),
                table: "table_name".to_owned(),
                column: "column_name".to_owned(),
            }]),
            cte_names: hash_set(&["cte_ident"]),
            as_clauses: hash_set(&["as_ident1", "as_ident2"]),
            last_tokens: VecDeque::from([TokenType::StartOfStatement, TokenType::Other]),
            last_schema_period: None,
            last_table_period: None,
        }
    );
}

#[test]
fn test_quoted_ident() {
    let result = complete(
        &setup(),
        r#"
WITH
    [ident1] AS (SELECT 1),
    `ident2` AS (SELECT 1),
    "ident3" AS (SELECT 1)
SELECT
    1 AS [ident4],
    1 AS `ident5`,
    1 AS "ident6""#,
        &loc(3, 0),
    );
    assert_eq!(result.cte_names, hash_set(&["ident1", "ident2", "ident3"]));
    assert_eq!(result.as_clauses, hash_set(&["ident4", "ident5", "ident6"]));
}

#[test]
fn test_as_clause_without_as() {
    let result = complete(
        &setup(),
        r#"
SELECT
    1 ident1,
    1 "ident2",
    +10 ident3,
    -10 ident4,
    'a' ident5,
    b + c ident6,
    (d < e) ident7,
    x'ff' ident8"#,
        &loc(3, 0),
    );
    assert_eq!(
        result.as_clauses,
        hash_set(&["ident1", "ident2", "ident3", "ident4", "ident5", "ident6", "ident7", "ident8"])
    );
}

#[test]
fn test_json_encode() {
    dbg!(serde_json::to_string(&complete(
        &setup(),
        r#"
WITH
    [ident1] AS (SELECT 1),
    `ident2` AS (SELECT 1),
    "ident3" AS (SELECT 1)
SELECT
    1 AS [ident4],
    1 AS `ident5`,
    1 AS "ident6",
    1 AS (SELECT 1)"#,
        &loc(3, 0),
    ))
    .unwrap());
}

fn is_start_of_stmt(c: Completions) {
    assert_eq!(c.last_tokens, VecDeque::from([TokenType::StartOfStatement]));
}

fn is_not_start_of_stmt(c: Completions) {
    assert_ne!(c.last_tokens, VecDeque::from([TokenType::StartOfStatement]));
}

#[test]
fn test_special_case_start_of_statement() {
    is_start_of_stmt(complete(&setup(), r#""#, &loc(0, 0)));
    is_start_of_stmt(complete(&setup(), r#"SELECT"#, &loc(0, 0)));
    is_start_of_stmt(complete(&setup(), r#"SELECT"#, &loc(0, 3)));
    is_start_of_stmt(complete(&setup(), r#"SELECT"#, &loc(0, 6)));
    is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 0)));
    is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 1)));
    is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 3)));
    is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 3)));
    is_not_start_of_stmt(complete(&setup(), r#"SEL "#, &loc(0, 4)));
    is_not_start_of_stmt(complete(&setup(), r#"SEL a"#, &loc(0, 5)));
    is_not_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 8)));
    is_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 9)));
    is_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 10)));
    is_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 11)));
}

fn is_after_join(mut c: Completions) {
    assert_eq!(c.last_tokens.pop_back(), Some(TokenType::JOIN));
}

fn is_not_after_join(mut c: Completions) {
    assert_ne!(c.last_tokens.pop_back(), Some(TokenType::JOIN));
}

#[test]
fn test_special_case_after_join() {
    is_not_after_join(complete(&setup(), r#"SELECT * FROM t JOIN"#, &loc(0, 19)));
    is_not_after_join(complete(&setup(), r#"SELECT * FROM t JOIN"#, &loc(0, 20)));
    is_after_join(complete(&setup(), r#"SELECT * FROM t JOIN "#, &loc(0, 21)));
    is_after_join(complete(&setup(), r#"SELECT * FROM t JOIN a"#, &loc(0, 21)));
    is_after_join(complete(&setup(), r#"SELECT * FROM t JOIN a"#, &loc(0, 22)));
}

fn is_after_schema_period(mut c: Completions) {
    assert_eq!(
        (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
        (Some(TokenType::Period), Some(TokenType::SchemaIdent))
    );
}

fn is_not_after_schema_period(mut c: Completions) {
    assert_ne!(
        (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
        (Some(TokenType::Period), Some(TokenType::SchemaIdent))
    );
}

#[test]
fn test_after_schema_period() {
    is_after_schema_period(complete(&setup(), r#"SELECT * FROM main."#, &loc(0, 19)));
    is_after_schema_period(complete(&setup(), r#"SELECT * FROM main. "#, &loc(0, 19)));
    is_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 19)));
    is_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 20)));
    is_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 21)));
    is_not_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 22)));
}

fn is_after_table_period(mut c: Completions) {
    assert_eq!(
        (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
        (Some(TokenType::Period), Some(TokenType::TableIdent))
    );
}

fn is_not_after_table_period(mut c: Completions) {
    assert_ne!(
        (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
        (Some(TokenType::Period), Some(TokenType::TableIdent))
    );
}

#[test]
fn test_after_table_period() {
    is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd."#, &loc(0, 19)));
    is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd. "#, &loc(0, 19)));
    is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 19)));
    is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 20)));
    is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 21)));
    is_not_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 22)));
}

fn is_after_as_paren(mut c: Completions) {
    assert_eq!(
        (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
        (Some(TokenType::LParen), Some(TokenType::AS))
    );
}

fn is_not_after_as_paren(mut c: Completions) {
    assert_ne!(
        (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
        (Some(TokenType::LParen), Some(TokenType::AS))
    );
}

#[test]
fn test_after_as_paren() {
    is_not_after_as_paren(complete(&setup(), r#"WITH a AS ("#, &loc(0, 10)));
    is_after_as_paren(complete(&setup(), r#"WITH a AS ("#, &loc(0, 11)));
    is_after_as_paren(complete(&setup(), r#"WITH a AS (a"#, &loc(0, 12)));
}

#[test]
fn test_temp_schema() {
    let mut result = complete(&setup(), r#"SELECT * FROM temp."#, &loc(0, 19));
    assert_eq!(result.last_tokens.pop_back(), Some(TokenType::Period));
    assert_eq!(result.last_tokens.pop_back(), Some(TokenType::SchemaIdent));
    assert_eq!(result.last_schema_period, Some("temp".to_owned()));
}

#[test]
fn test_nocase() {
    assert_eq!(
        complete(&setup(), r#"SELECT "Table_name". FROM "Table_name""#, &loc(0, 20))
            .columns_in_tables_that_are_referenced_in_source,
        HashSet::from([ColumnCompletion {
            schema: "main".to_owned(),
            table: "table_name".to_owned(),
            column: "column_name".to_owned(),
        }])
    );
}

#[test]
fn test_group() {
    assert_eq!(
        complete(&setup(), r#"INSERT INTO t (1, 2)"#, &loc(0, 20)).last_tokens,
        vec![
            TokenType::StartOfStatement,
            TokenType::INSERT,
            TokenType::INTO,
            TokenType::TableIdent,
            TokenType::Group
        ]
    );
}
