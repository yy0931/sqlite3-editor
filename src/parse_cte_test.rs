use crate::{
    parse_cte::parse_cte,
    split_statements::{get_text_range, split_sqlite_statements},
};

#[derive(Debug, PartialEq, Eq)]
struct CTEEntryString {
    ident: String,
    query: String,
}

impl CTEEntryString {
    fn new(ident: &str, query: &str) -> Self {
        Self {
            ident: ident.to_owned(),
            query: query.to_owned(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CTEString {
    entries: Vec<CTEEntryString>,
    body: String,
}

impl CTEString {
    fn new(entries: Vec<CTEEntryString>, body: &str) -> Self {
        Self {
            entries,
            body: body.to_owned(),
        }
    }
}

fn parse_cte_then_slice_string(sql: &str) -> CTEString {
    let cte = parse_cte(&split_sqlite_statements(sql).unwrap()[0]).unwrap();
    let lines = sql.lines().collect::<Vec<_>>();

    // Convert positions into strings
    CTEString {
        entries: cte
            .entries
            .iter()
            .map(|e| CTEEntryString {
                ident: get_text_range(&lines, &e.ident_start, &e.ident_end),
                query: get_text_range(&lines, &e.query_start, &e.query_end),
            })
            .collect::<Vec<_>>(),
        body: get_text_range(&lines, &cte.body_start, &cte.body_end),
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
