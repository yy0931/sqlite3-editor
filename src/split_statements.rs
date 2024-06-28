use std::borrow::Cow;

use sqlparser::{
    dialect::SQLiteDialect,
    keywords::Keyword,
    tokenizer::{Token, TokenizerError, Word},
};

use crate::tokenize::{tokenize_with_range_location, TokenWithRangeLocation, ZeroIndexedLocation};

/// Represents a split SQL statement with its text, actual text, and their locations.
/// ```plaintext
/// real_start  real_end
///    v        v
/// "  SELECT 1;  "
///  ^            ^
/// start        end
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct SplittedStatement {
    pub text: String,
    pub real_text: String,
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
    pub real_start: ZeroIndexedLocation,
    pub real_end: ZeroIndexedLocation,
    pub real_tokens: Vec<TokenWithRangeLocation>,
}

impl std::fmt::Debug for SplittedStatement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{
    raw:  {}:{}-{}:{} {:?}
    real: {}:{}-{}:{} {:?}
}}",
            self.start.line,
            self.start.column,
            self.end.line,
            self.end.column,
            self.text,
            self.real_start.line,
            self.real_start.column,
            self.real_end.line,
            self.real_end.column,
            self.real_text
        )
    }
}

impl SplittedStatement {
    fn new(
        lines: &[&str],
        tokens: &[&TokenWithRangeLocation],
        start: ZeroIndexedLocation,
        end: ZeroIndexedLocation,
    ) -> Self {
        // Get the position of the first non-whitespace token.
        let real_start_i = tokens
            .iter()
            .position(|token| !matches!(token.token, Token::Whitespace(_)));

        // Get the position of the last non-whitespace token.
        let real_end_i = tokens
            .iter()
            .rev()
            .position(|token| !matches!(token.token, Token::Whitespace(_)))
            .map(|i| tokens.len() - 1 - i);

        if let (Some(real_start_i), Some(real_end_i)) = (real_start_i, real_end_i) {
            let real_start = tokens[real_start_i].start.to_owned();
            let real_end = tokens[real_end_i].end.to_owned();
            Self {
                text: get_text_range(lines, &start, &end),
                real_text: get_text_range(lines, &real_start, &real_end),
                start,
                end,
                real_start: real_start.to_owned(),
                real_end: real_end.to_owned(),
                real_tokens: tokens[real_start_i..(real_end_i + 1)]
                    .iter()
                    .map(|&t| t.to_owned())
                    .collect::<Vec<_>>(),
            }
        } else {
            Self {
                text: get_text_range(lines, &start, &end),
                real_text: get_text_range(lines, &start, &end),
                start: start.clone(),
                end: end.clone(),
                real_start: start,
                real_end: end,
                real_tokens: tokens.iter().map(|&t| t.to_owned()).collect::<Vec<_>>(),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DotCommandArgument {
    pub arg_text: String,
    /// None if invalid
    pub arg_value: Option<String>,
    pub space_after_arg: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
            format!("'{}'", literal)
        } else {
            text.to_owned()
        };
        (literal, value)
    }

    /// Scans and parses the next double-quoted string literal in the provided text.
    /// This function returns the literal and its evaluated value.
    pub fn scan_double_quote_string_literal(text: &str) -> (String, Option<String>) {
        assert!(text.starts_with("\""));

        #[derive(PartialEq, Eq)]
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
            format!("\"{}\"", literal)
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

/// Splits the input SQL string into a vector of `SplittedStatement`.
pub fn split_sqlite_statements(sql: &str) -> Result<(Vec<SplittedStatement>, Vec<DotCommand>), TokenizerError> {
    let lines = sql.lines().collect::<Vec<_>>();

    // Tokenize the query
    let tokens = tokenize_with_range_location(&SQLiteDialect {}, sql)?;

    // Find dot-commands (https://sqlite.org/cli.html#rules_for_dot_commands_sql_and_more)
    let mut dot_commands = Vec::<DotCommand>::new();
    let mut tokens_excluding_dot_commands = vec![];
    {
        #[derive(PartialEq, Eq)]
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
                    if token.token == Token::Period && token.start.column == 0 {
                        last_dot_command_line = token.start.line;
                        dot_commands.push(DotCommand::new(token.start.line, lines[token.start.line]));
                        state = State::DotCommand;
                    } else if !matches!(token.token, Token::Whitespace(_)) {
                        state = State::SQLStatement;
                    }
                }
                State::DotCommand => {
                    if token.end.line > last_dot_command_line {
                        state = State::Default;
                    }
                }
                State::SQLStatement => {
                    if token.token == Token::SemiColon {
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
    let mut result: Vec<SplittedStatement> = vec![];

    // BEGIN ... (END or COMMIT or ROLLBACK) https://www.sqlite.org/lang_transaction.html
    let mut begin_end_block_depth = 0;

    // Split the token list at semicolons
    for token_with_location in &tokens {
        stmt_tokens.push(token_with_location);
        let TokenWithRangeLocation { token, end, .. } = token_with_location;
        match token {
            Token::Word(Word {
                keyword: Keyword::BEGIN | Keyword::CASE,
                ..
            }) => {
                begin_end_block_depth += 1;
            }
            Token::Word(Word {
                keyword: Keyword::END | Keyword::COMMIT | Keyword::ROLLBACK,
                ..
            }) => {
                begin_end_block_depth -= 1;
                if begin_end_block_depth < 0 {
                    begin_end_block_depth = 0;
                }
            }
            Token::SemiColon if begin_end_block_depth == 0 => {
                result.push(SplittedStatement::new(&lines, &stmt_tokens, stmt_start, end.to_owned()));
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
        if stmt_start != last.end {
            result.push(SplittedStatement::new(
                &lines,
                &stmt_tokens,
                stmt_start,
                last.end.clone(),
            ));
        }
    }

    Ok((result, dot_commands))
}

/// Extracts the text between the provided start and end locations from the given lines.
///
/// # Arguments
/// * `lines` - A slice of string slices representing lines of text.
/// * `start` - The start location represented as ZeroIndexedLocation.
/// * `end` - The end location represented as ZeroIndexedLocation.
pub fn get_text_range(lines: &[&str], start: &ZeroIndexedLocation, end: &ZeroIndexedLocation) -> String {
    if start.line == end.line {
        // If start and end are on the same line
        slice_unicode_str(lines[start.line], Some(start.column), Some(end.column))
    } else {
        // If start and end are on different lines
        let mut result: Vec<Cow<str>> = vec![];
        // Add the rest of the first line
        result.push(Cow::Owned(slice_unicode_str(
            lines[start.line],
            Some(start.column),
            None,
        )));
        // Add the complete lines between start and end
        for line in &lines[start.line + 1..end.line] {
            result.push(Cow::Borrowed(line));
        }
        // Add the part of the last line
        result.push(Cow::Owned(slice_unicode_str(lines[end.line], None, Some(end.column))));
        result.join("\n")
    }
}

/// Returns a Unicode-aware substring of the given string `s` using the specified start and end indices.
/// If start or end is `None`, it defaults to the start or end of the string respectively.
///
/// # Arguments
/// * `s` - A string slice that you want to get a substring of.
/// * `start` - Optional index for where the substring starts.
/// * `end` - Optional index for where the substring ends.
fn slice_unicode_str(s: &str, start: Option<usize>, end: Option<usize>) -> String {
    let start = start.unwrap_or(0);
    let end = end.unwrap_or_else(|| s.chars().count());
    s.chars()
        .skip(start)
        .take(
            end.checked_sub(start)
                .expect("Invalid range: start index is greater than end index."),
        )
        .collect()
}
