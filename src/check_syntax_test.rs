use crate::check_syntax::{check_syntax, Diagnostic, PossibleCause};

#[test]
fn test_simple_pass() {
    assert_eq!(check_syntax("DELETE FROM t"), Ok(vec![]));
}

#[test]
fn test_syntax_error() {
    assert_eq!(
        check_syntax("DELETE t"),
        Ok(vec![Diagnostic {
            possible_causes: vec![PossibleCause { offset: 7, length: 1 },],
            message: "near \"t\": syntax error".to_owned(),
        }]),
    );
}

#[test]
fn test_unterminated_string_literal() {
    assert_eq!(
        check_syntax(r#"SELECT 'a"#),
        Ok(vec![Diagnostic {
            possible_causes: vec![PossibleCause { offset: 7, length: 1 },],
            message: "Unterminated string literal".to_owned(),
        }])
    );
}

#[test]
fn test_multiple_errors() {
    assert_eq!(
        check_syntax("foo; bar;"),
        Ok(vec![
            Diagnostic {
                possible_causes: vec![PossibleCause { offset: 0, length: 1 },],
                message: "near \"foo\": syntax error".to_owned(),
            },
            Diagnostic {
                possible_causes: vec![PossibleCause { offset: 5, length: 1 },],
                message: "near \"bar\": syntax error".to_owned(),
            },
        ]),
    );
}

#[test]
fn test_ignore_errors_in_pragma_stmt() {
    assert_eq!(check_syntax("PRAGMA foo bar"), Ok(vec![]));
}

#[test]
fn test_pragma_functions() {
    assert_eq!(check_syntax("SELECT * FROM pragma_index_info(;").unwrap().len(), 1);
}

#[test]
fn test_ignore_errors_in_explain_stmt() {
    assert_eq!(check_syntax("EXPLAIN foo bar"), Ok(vec![]));
    assert_eq!(check_syntax("EXPLAIN QUERY PLAN foo bar"), Ok(vec![]));
}

#[test]
fn test_empty_input() {
    assert_eq!(check_syntax(";;"), Ok(vec![]));
}

#[test]
fn test_unknown_table_option() {
    assert_eq!(
        check_syntax("CREATE TABLE t(c) foobar"),
        Ok(vec![Diagnostic {
            possible_causes: vec![PossibleCause { offset: 24, length: 1 }],
            message: "unknown table option: foobar".to_owned()
        }])
    );
}
