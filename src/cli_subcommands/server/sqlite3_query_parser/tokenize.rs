use std::str::FromStr;

use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Keyword;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3KeywordLikeIdentifier;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Operator;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3PlaceholderKind;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::WithZeroIndexedRange;

use super::types::ZeroIndexedLocation;
use super::types::ZeroIndexedRange;
use once_cell::sync::Lazy;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::tokenizer::Location;
use sqlparser::tokenizer::Token;
use sqlparser::tokenizer::TokenWithLocation;
use sqlparser::tokenizer::Tokenizer;
use sqlparser::tokenizer::TokenizerError;
use sqlparser::tokenizer::Whitespace;

static HEXADECIMAL_NUMERIC_LITERAL: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r#"^\d+(_\d+)*$"#).unwrap());
static NUMERIC_LITERAL_CONTINUATION: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r#"^(_\d+)+([eE](\d+(_\d+)*)?)?$"#).unwrap());
static HEXADECIMAL_LITERAL_CONTINUATION: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r#"^X\d+(_\d+)*$"#).unwrap());

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZeroIndexedTokenizerError {
    pub location: ZeroIndexedLocation,
    pub message: String,
}

/// Tokenizes the given SQL input string and appends the end location to each token.
pub fn tokenize_with_range_location(
    sql: &str,
) -> Result<Vec<WithZeroIndexedRange<SQLite3Token>>, ZeroIndexedTokenizerError> {
    let raw_tokens = match tokenize_with_range_location_raw_sqlparser(sql) {
        Ok(v) => v,
        Err(TokenizerError { line, col, message }) => {
            return Err(ZeroIndexedTokenizerError {
                location: Location { line, column: col }.into(),
                message,
            });
        }
    };

    // Map the sqlparser's token to SQLite3Token
    let mut sqlite3_tokens: Vec<WithZeroIndexedRange<SQLite3Token>> = vec![];

    let mut dummy_range = WithZeroIndexedRange {
        value: SQLite3Token::Whitespace(false),
        range: ZeroIndexedRange::new4(0, 0, 0, 0),
    };

    for raw_token in raw_tokens {
        // Merge tokens
        {
            let last_token = sqlite3_tokens.last_mut().unwrap_or(&mut dummy_range);

            if let Some(merged_token) = merge_tokens(last_token, &raw_token) {
                last_token.value = merged_token;
                last_token.range.end = raw_token.range.end;
                continue;
            }
        }

        sqlite3_tokens.push(WithZeroIndexedRange {
            range: raw_token.range,
            value: match raw_token.value {
                Token::Number(s, /* "L" suffix */ false) => SQLite3Token::NumericLiteral(s),
                Token::Number(_, /* "L" suffix */ true) => SQLite3Token::InvalidNumericLiteral,
                Token::HexStringLiteral(s) if HEXADECIMAL_NUMERIC_LITERAL.is_match(&s) => {
                    SQLite3Token::NumericLiteral(format!("0x{s}"))
                }
                Token::Word(w) => {
                    if w.quote_style.is_some() {
                        // "quoted"
                        SQLite3Token::Identifier(w.value, None)
                    } else if w.value.starts_with(":") {
                        // :VVV
                        SQLite3Token::Placeholder(w.value, SQLite3PlaceholderKind::ColonName)
                    } else if w.value.starts_with("@") {
                        // @VVV
                        SQLite3Token::Placeholder(w.value, SQLite3PlaceholderKind::AtName)
                        // $VVV
                    } else if w.value.starts_with("$") {
                        SQLite3Token::Placeholder(w.value, SQLite3PlaceholderKind::DollarName)
                    } else if let Ok(kwd) = SQLite3Keyword::from_str(&w.value.to_uppercase()) {
                        // keyword
                        SQLite3Token::Keyword(kwd)
                    } else {
                        // identifier
                        let keyword_like = SQLite3KeywordLikeIdentifier::from_str(&w.value.to_uppercase()).ok();
                        SQLite3Token::Identifier(w.value, keyword_like)
                    }
                }

                Token::Placeholder(s) => {
                    if s == "?" {
                        SQLite3Token::Placeholder(s, SQLite3PlaceholderKind::Question)
                    } else if QUESTION_NUMBER.is_match(&s) {
                        SQLite3Token::Placeholder(s, SQLite3PlaceholderKind::QuestionNumber)
                    } else {
                        SQLite3Token::InvalidToken
                    }
                }
                Token::Colon => SQLite3Token::Placeholder(":".to_owned(), SQLite3PlaceholderKind::ColonName),
                Token::AtSign => SQLite3Token::Placeholder("@".to_owned(), SQLite3PlaceholderKind::AtName),

                Token::SingleQuotedString(s) => SQLite3Token::StringLiteral(s),
                Token::HexStringLiteral(s) => SQLite3Token::BlobLiteral(s),

                Token::DollarQuotedString(_)
                | Token::SingleQuotedByteStringLiteral(_)
                | Token::DoubleQuotedByteStringLiteral(_)
                | Token::RawStringLiteral(_)
                | Token::NationalStringLiteral(_)
                | Token::EscapedStringLiteral(_) => SQLite3Token::InvalidStringLiteral,

                Token::DoubleQuotedString(s) => SQLite3Token::Identifier(s, None),
                Token::Whitespace(Whitespace::SingleLineComment { .. })
                | Token::Whitespace(Whitespace::MultiLineComment { .. }) => SQLite3Token::Whitespace(true),

                Token::DoubleEq => SQLite3Token::Operator(SQLite3Operator::DoubleEq),
                Token::Eq => SQLite3Token::Operator(SQLite3Operator::Eq),
                Token::Neq => SQLite3Token::Operator(SQLite3Operator::Neq),
                Token::Lt => SQLite3Token::Operator(SQLite3Operator::Lt),
                Token::Gt => SQLite3Token::Operator(SQLite3Operator::Gt),
                Token::LtEq => SQLite3Token::Operator(SQLite3Operator::LtEq),
                Token::GtEq => SQLite3Token::Operator(SQLite3Operator::GtEq),
                Token::Plus => SQLite3Token::Operator(SQLite3Operator::Plus),
                Token::Minus => SQLite3Token::Operator(SQLite3Operator::Minus),
                Token::Mul => SQLite3Token::Operator(SQLite3Operator::Mul),
                Token::Div => SQLite3Token::Operator(SQLite3Operator::Div),
                Token::Mod => SQLite3Token::Operator(SQLite3Operator::Mod),
                Token::StringConcat => SQLite3Token::Operator(SQLite3Operator::StringConcat),
                Token::Ampersand => SQLite3Token::Operator(SQLite3Operator::Ampersand),
                Token::Pipe => SQLite3Token::Operator(SQLite3Operator::Pipe),
                Token::Tilde => SQLite3Token::Operator(SQLite3Operator::Tilde),
                Token::ShiftLeft => SQLite3Token::Operator(SQLite3Operator::ShiftLeft),
                Token::ShiftRight => SQLite3Token::Operator(SQLite3Operator::ShiftRight),
                Token::Arrow => SQLite3Token::Operator(SQLite3Operator::Arrow),
                Token::LongArrow => SQLite3Token::Operator(SQLite3Operator::LongArrow),

                Token::Spaceship
                | Token::DuckIntDiv
                | Token::DoubleColon
                | Token::Backslash
                | Token::Caret
                | Token::RArrow
                | Token::Sharp
                | Token::TildeAsterisk
                | Token::ExclamationMarkTilde
                | Token::ExclamationMarkTildeAsterisk
                | Token::ExclamationMark
                | Token::DoubleExclamationMark
                | Token::PGSquareRoot
                | Token::PGCubeRoot
                | Token::HashArrow
                | Token::HashLongArrow
                | Token::AtArrow
                | Token::ArrowAt
                | Token::HashMinus
                | Token::AtQuestion
                | Token::AtAt
                | Token::DuckAssignment
                | Token::Overlap
                | Token::LBracket
                | Token::RBracket
                | Token::LBrace
                | Token::RBrace => SQLite3Token::InvalidOperator,
                Token::Char(_) => SQLite3Token::InvalidToken,

                Token::Comma => SQLite3Token::Comma,
                Token::LParen => SQLite3Token::LParen,
                Token::RParen => SQLite3Token::RParen,
                Token::Period => SQLite3Token::Period,
                Token::SemiColon => SQLite3Token::SemiColon,

                Token::Whitespace(Whitespace::Newline | Whitespace::Space | Whitespace::Tab) => {
                    SQLite3Token::Whitespace(false)
                }

                Token::EOF => continue,
            },
        });
    }
    Ok(sqlite3_tokens)
}

/// Checks whether `raw_token`, which immediately follows `last_token`, should be merged into `last_token`.
/// Returns the merged token if the tokens should be merged, otherwise returns `None`.
fn merge_tokens(
    last_token: &WithZeroIndexedRange<SQLite3Token>,
    raw_token: &WithZeroIndexedRange<Token>,
) -> Option<SQLite3Token> {
    if last_token.range.end != raw_token.range.start {
        return None;
    }

    use SQLite3Token::*;
    match &last_token.value {
        NumericLiteral(s1) => {
            match &raw_token.value {
                Token::Word(w) if w.quote_style.is_none() => {
                    // Merge [Number("123"), Word("_456e")] into Number("123_456e")
                    if NUMERIC_LITERAL_CONTINUATION.is_match(&w.value) {
                        return Some(NumericLiteral(s1.clone() + &w.value));
                    }

                    // Merge [Number("0"), Word("x123")] into Number("0x123")
                    if s1 == "0" && HEXADECIMAL_LITERAL_CONTINUATION.is_match(&w.value) {
                        return Some(NumericLiteral(s1.clone() + &w.value));
                    }
                }

                // Merge [Number("123e"), Plus("+")] into Number("123e+")
                Token::Minus | Token::Plus if s1.ends_with("e") || s1.ends_with("E") => {
                    return Some(NumericLiteral(format!("{s1}{}", raw_token.value)));
                }

                // Merge [Number("123e+"), Number("4")] into Number("123e+4")
                Token::Number(s2, /* "L" suffix */ false) => {
                    return Some(NumericLiteral(format!("{s1}{s2}")));
                }
                _ => {}
            }
        }
        Placeholder(s1, kind) => {
            use SQLite3PlaceholderKind::*;
            if matches!(*kind, AtName | ColonName | DollarName) {
                match &raw_token.value {
                    // Merge [Placeholder("@"), Word("foo")] into Placeholder("@foo")
                    Token::Word(s2) if s2.quote_style.is_none() => {
                        return Some(Placeholder(format!("{s1}{}", s2.value), kind.clone()));
                    }

                    // Merge [Placeholder("@foo"), Number("123")] into Placeholder("@foo123")
                    Token::Number(s2, /* "L" suffix */ false) => {
                        return Some(Placeholder(format!("{s1}{s2}"), kind.clone()));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    None
}

static QUESTION_NUMBER: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"^\?\d+$").unwrap());

#[cfg(test)]
mod test_tokenize {
    use strum::IntoEnumIterator;

    use super::tokenize_with_range_location;
    use super::SQLite3Keyword;
    use super::SQLite3Keyword::*;
    use super::SQLite3KeywordLikeIdentifier;
    use super::SQLite3Token;
    use super::SQLite3Token::*;

    fn test(sql: &str, expectation: Vec<SQLite3Token>) {
        assert_eq!(
            tokenize_with_range_location(sql)
                .unwrap()
                .iter()
                .cloned()
                .map(|v| v.value)
                .collect::<Vec<_>>(),
            expectation
        )
    }

    #[test]
    fn test_keywords() {
        // upper case
        for kwd in SQLite3Keyword::iter() {
            test(
                &format!("{} 1", kwd.as_ref()),
                vec![Keyword(kwd), Whitespace(false), NumericLiteral("1".to_owned())],
            );
        }

        // lower case
        for kwd in SQLite3Keyword::iter() {
            test(
                &format!("{} 1", kwd.as_ref().to_lowercase()),
                vec![Keyword(kwd), Whitespace(false), NumericLiteral("1".to_owned())],
            );
        }
    }

    fn test_numeric_literal(literal: &str) {
        test(
            &format!("SELECT {literal}"),
            vec![Keyword(SELECT), Whitespace(false), NumericLiteral(literal.to_owned())],
        );
        test(
            &format!("SELECT {literal} 0"),
            vec![
                Keyword(SELECT),
                Whitespace(false),
                NumericLiteral(literal.to_owned()),
                Whitespace(false),
                NumericLiteral("0".to_owned()),
            ],
        );
    }

    #[test]
    fn test_decimal_numeric_literals() {
        for part1 in ["00", "00_00_00"] {
            test_numeric_literal(part1);
            test_numeric_literal(&format!(".{part1}"));
            for part2 in ["00", "00_00_00"] {
                test_numeric_literal(&format!("{part1}.{part2}"));
                for e in ["e", "E"] {
                    test_numeric_literal(&format!(".{part1}{e}{part2}"));
                    test_numeric_literal(&format!(".{part1}{e}+{part2}"));
                    test_numeric_literal(&format!(".{part1}{e}-{part2}"));

                    for part3 in ["00", "00_00_00"] {
                        test_numeric_literal(&format!("{part1}.{part2}{e}{part3}"));
                        test_numeric_literal(&format!("{part1}.{part2}{e}+{part3}"));
                        test_numeric_literal(&format!("{part1}.{part2}{e}-{part3}"));
                    }
                }
            }
        }
    }

    #[test]
    fn test_hex_numeric_literals() {
        test_numeric_literal("0x00");
        test_numeric_literal("0x00_00_00");
        test_numeric_literal("0X00");
        test_numeric_literal("0X00_00");
    }

    #[test]
    fn test_identifiers() {
        test(
            r#"SELECT "foo""bar", `foo"bar`"#,
            vec![
                Keyword(SELECT),
                Whitespace(false),
                Identifier("foo\"bar".to_owned(), None),
                Comma,
                Whitespace(false),
                Identifier("foo\"bar".to_owned(), None),
            ],
        );
    }

    #[test]
    fn test_keyword_like_identifiers() {
        test(
            r#"SELECT true, STRICT"#,
            vec![
                Keyword(SELECT),
                Whitespace(false),
                Identifier("true".to_owned(), Some(SQLite3KeywordLikeIdentifier::TRUE)),
                Comma,
                Whitespace(false),
                Identifier("STRICT".to_owned(), Some(SQLite3KeywordLikeIdentifier::STRICT)),
            ],
        );
    }
}

/// Tokenizes the given SQL input string with sqlparser and appends the end location to each token.
fn tokenize_with_range_location_raw_sqlparser(sql: &str) -> Result<Vec<WithZeroIndexedRange<Token>>, TokenizerError> {
    // Tokenize the SQL query
    let tokens = Tokenizer::new(&SQLiteDialect {}, sql).tokenize_with_location()?;

    // Merge whitespace tokens, e.g. [A, " ", " ", B] -> [A, "  ", B]
    let mut tokens_merged: Vec<TokenWithLocation> = vec![];
    for token in tokens {
        match token.token {
            Token::Whitespace(Whitespace::Space) | Token::Whitespace(Whitespace::Tab)
                if tokens_merged.last().map(|t| &t.token) == Some(&token.token) =>
            {
                continue
            }
            _ => {
                tokens_merged.push(token);
            }
        }
    }
    let tokens = tokens_merged;

    // Add range location to each token
    Ok(tokens
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, token)| {
            let start = token.location.into();
            let end = match tokens.get(i + 1) {
                Some(next_token) => next_token.location.clone().into(),
                None => {
                    // The location of the end of file
                    let lines = sql.lines().collect::<Vec<_>>();
                    ZeroIndexedLocation {
                        line: lines.len().saturating_sub(1),
                        column: lines.last().map_or(0, |line| line.len()),
                    }
                }
            };
            WithZeroIndexedRange::<Token> {
                value: token.token,
                range: ZeroIndexedRange { start, end },
            }
        })
        .collect::<Vec<_>>())
}

#[cfg(test)]
mod test_tokenize_raw {
    #[test]
    fn test_tokenize() {
        // Tokenize "CREATE TABLE t(c)"
        assert_eq!(
            super::tokenize_with_range_location_raw_sqlparser("CREATE TABLE t(c)")
                .unwrap()
                .into_iter()
                .map(|t| format!("{t:?}"))
                .collect::<Vec<_>>(),
            [
                r#"{0:0-0:6 Word(Word { value: "CREATE", quote_style: None, keyword: CREATE })}"#,
                r#"{0:6-0:7 Whitespace(Space)}"#,
                r#"{0:7-0:12 Word(Word { value: "TABLE", quote_style: None, keyword: TABLE })}"#,
                r#"{0:12-0:13 Whitespace(Space)}"#,
                r#"{0:13-0:14 Word(Word { value: "t", quote_style: None, keyword: NoKeyword })}"#,
                r#"{0:14-0:15 LParen}"#,
                r#"{0:15-0:16 Word(Word { value: "c", quote_style: None, keyword: NoKeyword })}"#,
                r#"{0:16-0:17 RParen}"#,
            ],
        );
    }

    #[test]
    fn test_merge_whitespace() {
        // Test that "   " is tokenized into a single token
        assert_eq!(
            super::tokenize_with_range_location_raw_sqlparser("SELECT   1;")
                .unwrap()
                .into_iter()
                .map(|t| format!("{t:?}"))
                .collect::<Vec<_>>(),
            [
                r#"{0:0-0:6 Word(Word { value: "SELECT", quote_style: None, keyword: SELECT })}"#,
                r#"{0:6-0:9 Whitespace(Space)}"#,
                r#"{0:9-0:10 Number("1", false)}"#,
                r#"{0:10-0:11 SemiColon}"#,
            ],
        );
    }
}
