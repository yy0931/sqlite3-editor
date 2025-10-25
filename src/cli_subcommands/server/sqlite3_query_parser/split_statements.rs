use super::types::WithZeroIndexedRange;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Keyword;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::tokenize::tokenize_with_range_location;
use crate::cli_subcommands::server::sqlite3_query_parser::tokenize::ZeroIndexedTokenizerError;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedLocation;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;

/// Splits the input SQL string into a vector of `SingleStatement`.
pub fn split_sqlite_statements(
    sql: &str,
) -> Result<(Vec<SingleStatement>, Vec<DotCommand>), ZeroIndexedTokenizerError> {
    let lines = sql.lines().collect::<Vec<_>>();

    // Tokenize the query
    let tokens = tokenize_with_range_location(sql)?;

    // Find dot-commands (https://sqlite.org/cli.html#rules_for_dot_commands_sql_and_more)
    let mut dot_commands = Vec::<DotCommand>::new();
    let mut tokens_excluding_dot_commands = vec![];
    {
        #[derive(Eq, PartialEq)]
        enum State {
            /// Outside of any SQL statements or dot commands.
            Default,
            /// Inside of a SQL statement.
            SQLStatement,
            /// Inside of a dot command.
            DotCommand,
        }

        let mut state = State::Default;
        let mut last_dot_command_line = 0;
        for token in tokens {
            match state {
                State::Default => {
                    if token.value == SQLite3Token::Period && token.range.start.column == 0 {
                        last_dot_command_line = token.range.start.line;
                        dot_commands.push(DotCommand::new(token.range.start.line, lines[token.range.start.line]));
                        state = State::DotCommand;
                    } else if !token.value.is_whitespace() {
                        state = State::SQLStatement;
                    }
                }
                State::DotCommand => {
                    if token.range.end.line > last_dot_command_line {
                        state = State::Default;
                    }
                }
                State::SQLStatement => {
                    if token.value == SQLite3Token::SemiColon {
                        state = State::Default;
                    }
                }
            }
            if state != State::DotCommand {
                tokens_excluding_dot_commands.push(token);
            }
        }
    }
    let tokens = tokens_excluding_dot_commands;

    let mut stmt_start = ZeroIndexedLocation { column: 0, line: 0 };
    let mut stmt_tokens = vec![];
    let mut result: Vec<SingleStatement> = vec![];

    // BEGIN ... (END or COMMIT or ROLLBACK) https://www.sqlite.org/lang_transaction.html
    let mut begin_end_block_depth = 0;

    // Split the token list at semicolons
    for token_with_location in &tokens {
        stmt_tokens.push(token_with_location);
        let end = &token_with_location.range.end;
        match token_with_location.value {
            SQLite3Token::Keyword(SQLite3Keyword::BEGIN | SQLite3Keyword::CASE) => {
                begin_end_block_depth += 1;
            }
            SQLite3Token::Keyword(SQLite3Keyword::END | SQLite3Keyword::COMMIT | SQLite3Keyword::ROLLBACK) => {
                begin_end_block_depth -= 1;
                if begin_end_block_depth < 0 {
                    begin_end_block_depth = 0;
                }
            }
            SQLite3Token::SemiColon if begin_end_block_depth == 0 => {
                result.push(SingleStatement::new(
                    &lines,
                    &stmt_tokens,
                    ZeroIndexedRange::new(stmt_start, end.to_owned()),
                ));
                #[allow(clippy::assigning_clones)] // cannot use clone_into because stmt_start has been moved
                {
                    stmt_start = end.to_owned();
                }
                stmt_tokens.clear();
            }
            _ => {}
        }
    }

    if let Some(last) = tokens.last() {
        if stmt_start != last.range.end {
            result.push(SingleStatement::new(
                &lines,
                &stmt_tokens,
                ZeroIndexedRange::new(stmt_start, last.range.end.clone()),
            ));
        }
    }

    Ok((result, dot_commands))
}

/// Represents a split SQL statement with its text, actual text, and their locations.
/// ```plaintext
/// real_text.start  real_text.end
///           v        v
///        "  SELECT 1;  "
///         ^            ^
/// all_text.start   all_text.end
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct SingleStatement {
    pub all_text: WithZeroIndexedRange<String>,
    pub real_text: WithZeroIndexedRange<String>,
    pub real_tokens: Vec<WithZeroIndexedRange<SQLite3Token>>,
}

impl std::fmt::Debug for SingleStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{
    raw:  {}:{}-{}:{} {:?}
    real: {}:{}-{}:{} {:?}
}}",
            self.all_text.range.start.line,
            self.all_text.range.start.column,
            self.all_text.range.end.line,
            self.all_text.range.end.column,
            self.all_text.value,
            self.real_text.range.start.line,
            self.real_text.range.start.column,
            self.real_text.range.end.line,
            self.real_text.range.end.column,
            self.real_text.value
        )
    }
}

impl SingleStatement {
    fn new(lines: &[&str], tokens: &[&WithZeroIndexedRange<SQLite3Token>], range: ZeroIndexedRange) -> Self {
        // Get the position of the first non-whitespace token.
        let real_start_i = tokens.iter().position(|token| !token.value.is_whitespace());

        // Get the position of the last non-whitespace token.
        let real_end_i = tokens
            .iter()
            .rev()
            .position(|token| !token.value.is_whitespace())
            .map(|i| tokens.len() - 1 - i);

        if let (Some(real_start_i), Some(real_end_i)) = (real_start_i, real_end_i) {
            let real_range = ZeroIndexedRange::new(
                tokens[real_start_i].range.start.to_owned(),
                tokens[real_end_i].range.end.to_owned(),
            );
            Self {
                all_text: range.get_text_with_range(lines),
                real_text: real_range.get_text_with_range(lines),
                real_tokens: tokens[real_start_i..(real_end_i + 1)]
                    .iter()
                    .map(|&t| t.to_owned())
                    .collect::<Vec<_>>(),
            }
        } else {
            let real_text = range.get_text_with_range(lines);
            Self {
                all_text: real_text.clone(),
                real_text,
                real_tokens: tokens.iter().map(|&t| t.to_owned()).collect::<Vec<_>>(),
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DotCommandArgument {
    pub arg_text: String,
    /// None if invalid
    pub arg_value: Option<String>,
    pub space_after_arg: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DotCommand {
    /// Zero-indexed line number.
    pub line: usize,
    pub text: String,

    pub name: String,
    pub space_after_name: String,
    pub args: Vec<DotCommandArgument>,
}

impl DotCommand {
    /// Scans and parses the next single-quoted string literal in the provided text.
    /// This function returns the literal and its evaluated value.
    pub fn scan_single_quote_string_literal(text: &str) -> (String, Option<String>) {
        assert!(text.starts_with("'"));

        let mut closed = false;
        let literal = text
            .chars()
            .skip(1)
            .take_while(|c| {
                closed = *c == '\'';
                !closed
            })
            .collect::<String>();
        let value = closed.then_some(literal.clone());
        let literal = if closed {
            format!("'{literal}'")
        } else {
            text.to_owned()
        };
        (literal, value)
    }

    /// Scans and parses the next double-quoted string literal in the provided text.
    /// This function returns the literal and its evaluated value.
    pub fn scan_double_quote_string_literal(text: &str) -> (String, Option<String>) {
        assert!(text.starts_with("\""));

        #[derive(Eq, PartialEq)]
        enum State {
            Default,
            /// \
            Escape,
            /// \x
            Hex,
            /// \0-\7
            Octal,
        }

        let mut closed = false;
        let mut state = State::Default;
        let mut hex_or_octal_digits = String::new();
        let mut arg_value_chars = String::new();

        let literal = text
            .chars()
            .skip(1)
            .take_while(|&c| {
                // Parse the \xff literal and the \777 literal
                if matches!(state, State::Hex | State::Octal) {
                    let is_digit = if state == State::Hex {
                        c.is_ascii_hexdigit()
                    } else {
                        matches!(c, '0'..='7')
                    };
                    if is_digit {
                        hex_or_octal_digits.push(c);
                    }
                    if !is_digit || hex_or_octal_digits.len() >= (if state == State::Hex { 2 } else { 3 }) {
                        let radix = if state == State::Hex { 16 } else { 8 };
                        let i = u32::from_str_radix(&hex_or_octal_digits, radix).unwrap_or_default();
                        arg_value_chars.push(char::from_u32(i).unwrap_or_default());
                        hex_or_octal_digits.clear();
                        state = State::Default;
                    }
                    if is_digit {
                        return true;
                    }
                }

                if state == State::Escape {
                    // https://github.com/sqlite/sqlite/blob/cf2710e0b61dbe5abd3b0017905dfc5cb6c21d8d/src/shell.c.in#L5577
                    arg_value_chars.push(match c {
                        'a' => '\x07',
                        'b' => '\x08',
                        't' => '\t',
                        'n' => '\n',
                        'v' => '\x0B',
                        'f' => '\x0C',
                        'r' => '\r',
                        '"' => '"',
                        '\'' => '\'',
                        '\\' => '\\',
                        'x' => {
                            // Read up to two characters and parse it as a hexadecimal literal
                            state = State::Hex;
                            return true;
                        }
                        '0'..='7' => {
                            // Read up to three characters and parse it as a octal literal
                            state = State::Octal;
                            hex_or_octal_digits.push(c);
                            return true;
                        }
                        _ => c,
                    });
                    state = State::Default;
                } else if c == '\\' {
                    state = State::Escape;
                } else if c == '"' {
                    closed = true;
                    return false;
                } else {
                    arg_value_chars.push(c);
                }
                true
            })
            .collect::<String>();

        let literal = if closed {
            format!("\"{literal}\"")
        } else {
            text.to_owned()
        };
        let value = closed.then_some(arg_value_chars);
        (literal, value)
    }

    pub fn new(line: usize, mut text: &str) -> Self {
        // https://sqlite.org/cli.html#dot_command_arguments

        let original_text = text.to_owned();

        // A dot
        assert!(text.starts_with('.'));
        text = &text[1..];

        // The command name
        let name = text.chars().take_while(|c| *c != ' ').collect::<String>();
        text = &text[name.len()..];

        // The whitespace characters after the command name.
        let space_after_name = text.chars().take_while(|c| *c == ' ').collect::<String>();
        text = &text[space_after_name.len()..];

        let mut args: Vec<DotCommandArgument> = vec![];
        while !text.is_empty() {
            // An argument
            let arg_text: String;
            let arg_value: Option<String>;
            match text.chars().nth(0).unwrap() {
                '\'' => {
                    (arg_text, arg_value) = Self::scan_single_quote_string_literal(text);
                    text = &text[arg_text.len()..];
                }
                '"' => {
                    (arg_text, arg_value) = Self::scan_double_quote_string_literal(text);
                    text = &text[arg_text.len()..];
                }
                _ => {
                    arg_text = text.chars().take_while(|c| *c != ' ').collect::<String>();
                    arg_value = Some(arg_text.clone());
                    text = &text[arg_text.len()..];
                }
            }

            // The whitespace characters after the argument.
            let space_after_arg = text.chars().take_while(|c| *c == ' ').collect::<String>();
            text = &text[space_after_arg.len()..];

            args.push(DotCommandArgument {
                arg_text,
                arg_value,
                space_after_arg,
            });
        }

        Self {
            line,
            text: original_text,
            name,
            space_after_name,
            args,
        }
    }
}

#[cfg(test)]
mod test {
    use super::split_sqlite_statements;
    use super::DotCommand;
    use super::DotCommandArgument;

    #[test]
    fn test_simple() {
        // Split "SELECT 1; SELECT 2;"
        assert_eq!(
            split_sqlite_statements("SELECT 1; SELECT 2;")
                .unwrap()
                .0
                .into_iter()
                .map(|t| format!("{t:?}"))
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
                .map(|t| format!("{t:?}"))
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
                .map(|t| format!("{t:?}"))
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
                .map(|t| format!("{t:?}"))
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
            statements.into_iter().map(|t| format!("{t:?}")).collect::<Vec<_>>(),
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
}
