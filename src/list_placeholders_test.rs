use crate::{
    list_placeholders::{list_placeholders, Placeholder, PlaceholderRange},
    split_statements::split_sqlite_statements,
    tokenize::ZeroIndexedLocation,
};

fn list_placeholders_with_sqlite_api(sql: &str) -> rusqlite::Result<Vec<Option<String>>> {
    let conn = rusqlite::Connection::open_in_memory()?;
    let stmt = conn.prepare(sql)?;
    Ok((1..=stmt.parameter_count())
        .map(|i| stmt.parameter_name(i).map(|v| v.to_owned()))
        .collect())
}

fn compare(sql: &str) {
    let expected = list_placeholders_with_sqlite_api(sql).unwrap();
    let actual = list_placeholders(&split_sqlite_statements(sql).unwrap().0[0]);
    assert_eq!(expected.len(), actual.len());
    for i in 0..expected.len() {
        assert_eq!(actual[i].name, expected[i]);
    }
}

#[test]
pub fn test_list_placeholders_without_comparing_to_sqlite_api_output() {
    assert_eq!(
        list_placeholders(&split_sqlite_statements("SELECT ?, $a").unwrap().0[0]),
        vec![
            Placeholder {
                name: None,
                ranges_relative_to_stmt: vec![PlaceholderRange {
                    start: ZeroIndexedLocation::new(0, 7),
                    end: ZeroIndexedLocation::new(0, 8),
                }],
            },
            Placeholder {
                name: Some("$a".to_owned()),
                ranges_relative_to_stmt: vec![PlaceholderRange {
                    start: ZeroIndexedLocation::new(0, 10),
                    end: ZeroIndexedLocation::new(0, 12),
                }],
            },
        ]
    );
}

#[test]
pub fn test_no_placeholder() {
    compare("SELECT 1");
}

#[test]
pub fn test_placeholder_reuse() {
    compare("SELECT ?, ?, @a, ?2, ?3");
}

#[test]
pub fn test_everything() {
    compare("WITH x AS (SELECT @a) SELECT ?, ?, ?10, :10, @10, $10, :aa, @aa, $aa, ?12, ?, :1a1");
}
