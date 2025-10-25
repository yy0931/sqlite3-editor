use crate::cli_error::CLIError;
use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::tokenize::tokenize_with_range_location;
use crate::cli_subcommands::server::sqlite3_query_parser::types::WithZeroIndexedRange;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedRange;
use crate::cli_subcommands::server::TruncateAll;
use rmp_serde::encode::write_named;
use serde::Deserialize;
use serde::Serialize;
use std::fs::File;
use std::io::Write;

pub fn run(r: &mut File, w: &mut File) -> CLIErrorCode {
    // Handle the command
    if let Err(err) = rmp_serde::from_read(r).map(|SemanticHighlightCommandParams { query }: SemanticHighlightCommandParams| -> std::result::Result<(), CLIError> {
        write_named(w, &semantic_highlight(&query))?;
        Ok(())
    }) {
        w.flush().unwrap();
        w.truncate_all();
        write!(w, "{err:?}").unwrap();
        CLIErrorCode::OtherError
    } else {
        w.flush().unwrap();
        CLIErrorCode::Success
    }
}

/// Structure representing a database query
#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(from = "(String,)")]
pub struct SemanticHighlightCommandParams {
    pub query: String,
}

impl From<(String,)> for SemanticHighlightCommandParams {
    fn from(value: (String,)) -> Self {
        Self { query: value.0 }
    }
}

/// Represents the kind of token highlighting.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub enum SemanticTokenKind {
    Keyword,
    Number,
    String,
    Operator,
    Comment,
    Function,
    Variable,
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SemanticHighlight {
    pub kind: SemanticTokenKind,
    pub range: ZeroIndexedRange,
}

/// Tokenizes the given SQL input string and returns the tokens with highlighting information.
pub fn semantic_highlight(sql: &str) -> Vec<SemanticHighlight> {
    let mut tokens = vec![];
    let Ok(parsed_tokens) = tokenize_with_range_location(sql) else {
        return tokens;
    };

    for WithZeroIndexedRange { value: token, range } in parsed_tokens {
        if range.start == range.end {
            continue;
        }
        use SQLite3Token::*;
        tokens.push(SemanticHighlight {
            kind: match token {
                NumericLiteral(_) | InvalidNumericLiteral => SemanticTokenKind::Number,
                StringLiteral(_) | BlobLiteral(_) | InvalidStringLiteral => SemanticTokenKind::String,
                Identifier(_, Some(_)) => SemanticTokenKind::Keyword,
                Identifier(_, None) => SemanticTokenKind::Variable,
                Keyword(_) => SemanticTokenKind::Keyword,
                Whitespace(true) => SemanticTokenKind::Comment,
                Operator(_) | InvalidOperator => SemanticTokenKind::Operator,
                Comma | Placeholder(_, _) | LParen | RParen | Period | SemiColon | Whitespace(false) | InvalidToken => {
                    SemanticTokenKind::Other
                }
            },
            range,
        });
    }
    tokens
}

#[cfg(test)]
mod test {
    use super::semantic_highlight;
    use super::SemanticTokenKind;

    #[test]
    fn test_token_kinds() {
        assert_eq!(
            semantic_highlight("SELECT 1 = 2 * a; /* b */ -- c")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                SemanticTokenKind::Keyword,  // "SELECT"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Number,   // "1"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Operator, // "="
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Number,   // "2"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Operator, // "*"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Variable, // "a"
                SemanticTokenKind::Other,    // ";"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Comment,  // "/* b */"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Comment,  // "-- c"
            ]
        );
    }

    #[test]
    fn test_quotes() {
        assert_eq!(
            semantic_highlight("SELECT 'a', \"b\", [c], `d`")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                SemanticTokenKind::Keyword,  // "SELECT"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::String,   // "'a'"
                SemanticTokenKind::Other,    // ","
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Variable, // "\"b\""
                SemanticTokenKind::Other,    // ","
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Variable, // "[c]"
                SemanticTokenKind::Other,    // ","
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Variable, // "`d`"
            ]
        );
    }

    #[test]
    fn test_attach_database() {
        assert_eq!(
            semantic_highlight("ATTACH DATABASE 'db' AS db;")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                SemanticTokenKind::Keyword,  // "ATTACH"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Keyword,  // "DATABASE"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::String,   // "'db'"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Keyword,  // "AS"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Variable, // "db"
                SemanticTokenKind::Other,    // ";"
            ]
        );
    }

    #[test]
    fn test_quoted_identifier() {
        assert_eq!(
            semantic_highlight("\"a\"")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [SemanticTokenKind::Variable]
        )
    }

    #[test]
    fn test_pragma() {
        assert_eq!(
            semantic_highlight("PRAGMA analysis_limit")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                SemanticTokenKind::Keyword,  // "PRAGMA"
                SemanticTokenKind::Other,    // " "
                SemanticTokenKind::Variable, // "analysis_limit"
            ]
        );
    }

    #[test]
    fn test_blob_literal() {
        assert_eq!(
            semantic_highlight("SELECT x'ff'")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                SemanticTokenKind::Keyword, // "SELECT"
                SemanticTokenKind::Other,   // " "
                SemanticTokenKind::String,  // "x'ff'"
            ]
        );
    }

    #[test]
    fn test_tokenizer_error() {
        assert_eq!(
            semantic_highlight("'aa")
                .into_iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            []
        );
    }

    fn assert_all_tokens_are_number(expr: &str) {
        let tokens = semantic_highlight(expr).into_iter().map(|t| t.kind).collect::<Vec<_>>();
        assert_eq!(
            tokens,
            tokens.iter().map(|_| SemanticTokenKind::Number).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_delimited_numeric_literal_integer_and_float() {
        for part1 in ["00", "00_00_00"] {
            assert_all_tokens_are_number(part1);
            assert_all_tokens_are_number(&format!(".{part1}"));
            for part2 in ["00", "00_00_00"] {
                assert_all_tokens_are_number(&format!("{part1}.{part2}"));
                for e in ["e", "E"] {
                    assert_all_tokens_are_number(&format!(".{part1}{e}{part2}"));
                    assert_all_tokens_are_number(&format!(".{part1}{e}+{part2}"));
                    assert_all_tokens_are_number(&format!(".{part1}{e}-{part2}"));

                    for part3 in ["00", "00_00_00"] {
                        assert_all_tokens_are_number(&format!("{part1}.{part2}{e}{part3}"));
                        assert_all_tokens_are_number(&format!("{part1}.{part2}{e}+{part3}"));
                        assert_all_tokens_are_number(&format!("{part1}.{part2}{e}-{part3}"));
                    }
                }
            }
        }
    }

    #[test]
    fn test_delimited_numeric_literal_hex() {
        assert_all_tokens_are_number("0x00");
        assert_all_tokens_are_number("0x00_00_00");
        assert_all_tokens_are_number("0X00");
        assert_all_tokens_are_number("0X00_00");
    }
}
