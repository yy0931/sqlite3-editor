use crate::split_statements::split_sqlite_statements;

#[test]
fn test_simple() {
    // Split "SELECT 1; SELECT 2;"
    assert_eq!(
        split_sqlite_statements("SELECT 1; SELECT 2;")
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{
    raw:  <line 0, column 0>-<line 0, column 9>: "SELECT 1;"
    real: <line 0, column 0>-<line 0, column 9>: "SELECT 1;"
}"#,
            r#"{
    raw:  <line 0, column 9>-<line 0, column 19>: " SELECT 2;"
    real: <line 0, column 10>-<line 0, column 19>: "SELECT 2;"
}"#
        ]
    );
}

#[test]
fn test_case_expression() {
    // Test the CASE expression
    assert_eq!(
        split_sqlite_statements(r#"SELECT CASE WHEN 1 THEN 2 ELSE 3 END; SELECT 4;"#)
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{
    raw:  <line 0, column 0>-<line 0, column 37>: "SELECT CASE WHEN 1 THEN 2 ELSE 3 END;"
    real: <line 0, column 0>-<line 0, column 37>: "SELECT CASE WHEN 1 THEN 2 ELSE 3 END;"
}"#,
            r#"{
    raw:  <line 0, column 37>-<line 0, column 47>: " SELECT 4;"
    real: <line 0, column 38>-<line 0, column 47>: "SELECT 4;"
}"#
        ]
    );
}

#[test]
fn test_unmatched_end() {
    // Split "SELECT 1; SELECT 2;"
    assert_eq!(
        split_sqlite_statements("END; SELECT 1;")
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{
    raw:  <line 0, column 0>-<line 0, column 4>: "END;"
    real: <line 0, column 0>-<line 0, column 4>: "END;"
}"#,
            r#"{
    raw:  <line 0, column 4>-<line 0, column 14>: " SELECT 1;"
    real: <line 0, column 5>-<line 0, column 14>: "SELECT 1;"
}"#
        ]
    );
}

#[test]
fn test_whitespace() {
    assert_eq!(
        split_sqlite_statements("    ")
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [r#"{
    raw:  <line 0, column 0>-<line 0, column 4>: "    "
    real: <line 0, column 0>-<line 0, column 4>: "    "
}"#,]
    );
}
