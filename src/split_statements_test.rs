use crate::split_statements::{split_sqlite_statements, DotCommand, DotCommandArgument};

#[test]
fn test_simple() {
    // Split "SELECT 1; SELECT 2;"
    assert_eq!(
        split_sqlite_statements("SELECT 1; SELECT 2;")
            .unwrap()
            .0
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{
    raw:  0:0-0:9 "SELECT 1;"
    real: 0:0-0:9 "SELECT 1;"
}"#,
            r#"{
    raw:  0:9-0:19 " SELECT 2;"
    real: 0:10-0:19 "SELECT 2;"
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
            .0
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{
    raw:  0:0-0:37 "SELECT CASE WHEN 1 THEN 2 ELSE 3 END;"
    real: 0:0-0:37 "SELECT CASE WHEN 1 THEN 2 ELSE 3 END;"
}"#,
            r#"{
    raw:  0:37-0:47 " SELECT 4;"
    real: 0:38-0:47 "SELECT 4;"
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
            .0
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{
    raw:  0:0-0:4 "END;"
    real: 0:0-0:4 "END;"
}"#,
            r#"{
    raw:  0:4-0:14 " SELECT 1;"
    real: 0:5-0:14 "SELECT 1;"
}"#
        ]
    );
}

#[test]
fn test_whitespace() {
    assert_eq!(
        split_sqlite_statements("    ")
            .unwrap()
            .0
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [r#"{
    raw:  0:0-0:4 "    "
    real: 0:0-0:4 "    "
}"#,]
    );
}

#[test]
fn test_dot_commands() {
    let (statements, dot_commands) = split_sqlite_statements(
        r#"SELECT 1; -- comment
.separator ", "
SELECT 2;"#,
    )
    .unwrap();

    assert_eq!(dot_commands, [DotCommand::new(1, r#".separator ", ""#)]);

    assert_eq!(
        statements.into_iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>(),
        [
            r#"{
    raw:  0:0-0:9 "SELECT 1;"
    real: 0:0-0:9 "SELECT 1;"
}"#,
            r#"{
    raw:  0:9-2:9 " -- comment\n.separator \", \"\nSELECT 2;"
    real: 2:0-2:9 "SELECT 2;"
}"#
        ]
    );
}

#[test]
fn test_dot_command() {
    let command = r#".name "ab" 'cd' ef"#;
    assert_eq!(
        DotCommand::new(0, command),
        DotCommand {
            line: 0,
            text: command.to_owned(),
            name: "name".to_owned(),
            space_after_name: " ".to_owned(),
            args: vec![
                DotCommandArgument {
                    arg_text: r#""ab""#.to_owned(),
                    space_after_arg: " ".to_owned(),
                    arg_value: Some("ab".to_owned()),
                },
                DotCommandArgument {
                    arg_text: "'cd'".to_owned(),
                    space_after_arg: " ".to_owned(),
                    arg_value: Some("cd".to_owned()),
                },
                DotCommandArgument {
                    arg_text: "ef".to_owned(),
                    space_after_arg: "".to_owned(),
                    arg_value: Some("ef".to_owned()),
                }
            ],
        }
    );
}

#[test]
fn test_empty_single_quote_string() {
    assert_eq!(
        DotCommand::scan_single_quote_string_literal("''foo"),
        ("''".to_owned(), Some("".to_owned()))
    );
}

#[test]
fn test_simple_single_quote_string() {
    assert_eq!(
        DotCommand::scan_single_quote_string_literal("'a'b"),
        ("'a'".to_owned(), Some("a".to_owned()))
    );
}

#[test]
fn test_unclosed_single_quote_string() {
    assert_eq!(
        DotCommand::scan_single_quote_string_literal("'a b"),
        ("'a b".to_owned(), None)
    );
}

#[test]
fn test_single_character_single_quote_string() {
    assert_eq!(
        DotCommand::scan_single_quote_string_literal("'a'bc"),
        ("'a'".to_owned(), Some("a".to_owned()))
    );
}

#[test]
fn test_escaped_single_quote_inside_string() {
    assert_eq!(
        DotCommand::scan_single_quote_string_literal("'a\\'b' c"),
        ("'a\\'".to_owned(), Some("a\\".to_owned()))
    );
}

#[test]
fn test_quote_at_end_of_string() {
    assert_eq!(
        DotCommand::scan_single_quote_string_literal("'a'"),
        ("'a'".to_owned(), Some("a".to_owned()))
    );
}

#[test]
fn test_empty_double_quote_string() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#"""foo"#),
        (r#""""#.to_owned(), Some("".to_owned()))
    );
}

#[test]
fn test_simple_string_literal() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#""hello""#),
        (r#""hello""#.to_owned(), Some("hello".to_owned()))
    );
}

#[test]
fn test_escaped_double_quote() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#""\"""#),
        (r#""\"""#.to_owned(), Some("\"".to_owned()))
    );
}

#[test]
fn test_hex_escape_sequence() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#""\x48\x65\x6C\x6C\x6F\x7\x""#),
        (
            r#""\x48\x65\x6C\x6C\x6F\x7\x""#.to_owned(),
            Some("Hello\u{7}\0".to_owned())
        )
    );
}

#[test]
fn test_octal_escape_sequence() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#""\101\102\103\1\0""#),
        (r#""\101\102\103\1\0""#.to_owned(), Some("ABC\u{1}\0".to_owned()))
    );
}

#[test]
fn test_unclosed_double_quote() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#""unclosed"#),
        (r#""unclosed"#.to_owned(), None)
    );
}

#[test]
fn test_literal_with_all_escapes() {
    assert_eq!(
        DotCommand::scan_double_quote_string_literal(r#""\a\b\t\n\v\f\r\"\'\\\x41\101""#),
        (
            r#""\a\b\t\n\v\f\r\"\'\\\x41\101""#.to_owned(),
            Some("\x07\x08\t\n\x0B\x0C\r\"'\\AA".to_owned())
        )
    );
}
