use serde::{Deserialize, Serialize};
use sqlparser::{
    dialect::SQLiteDialect,
    keywords::Keyword,
    tokenizer::{Token, Whitespace, Word},
};

use crate::tokenize::{tokenize_with_range_location, TokenWithRangeLocation, ZeroIndexedLocation};

/// Represents the kind of token highlighting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHighlight {
    pub kind: SemanticTokenKind,
    pub start: ZeroIndexedLocation,
    pub end: ZeroIndexedLocation,
}

/// Tokenizes the given SQL input string and returns the tokens with highlighting information.
pub fn semantic_highlight(sql: &str) -> Vec<SemanticHighlight> {
    let mut tokens = vec![];
    let Ok(parsed_tokens) = tokenize_with_range_location(&SQLiteDialect {}, sql) else {
        return tokens;
    };
    for TokenWithRangeLocation { token, start, end } in parsed_tokens {
        if start == end {
            continue;
        }
        tokens.push(SemanticHighlight {
            kind: match token {
                Token::EOF => SemanticTokenKind::Other,
                Token::Word(Word {
                    quote_style: None,
                    value,
                    keyword: Keyword::NoKeyword,
                }) if value == "VACUUM" || value == "ATTACH" || value == "DETACH" => SemanticTokenKind::Keyword,
                Token::Word(Word {
                    keyword: Keyword::NoKeyword,
                    ..
                }) => SemanticTokenKind::Variable,
                Token::Word(_) => SemanticTokenKind::Keyword,
                Token::Number(_string, _bool) => SemanticTokenKind::Number,
                Token::Char(_char) => SemanticTokenKind::Other,
                Token::SingleQuotedString(_string) => SemanticTokenKind::String,
                Token::DoubleQuotedString(_string) => SemanticTokenKind::Variable,
                Token::DollarQuotedString(_dollarquotedstring) => SemanticTokenKind::String,
                Token::SingleQuotedByteStringLiteral(_string) => SemanticTokenKind::String,
                Token::DoubleQuotedByteStringLiteral(_string) => SemanticTokenKind::String,
                Token::RawStringLiteral(_string) => SemanticTokenKind::String,
                Token::NationalStringLiteral(_string) => SemanticTokenKind::String,
                Token::EscapedStringLiteral(_string) => SemanticTokenKind::String,
                Token::HexStringLiteral(_string) => SemanticTokenKind::String,
                Token::Comma => SemanticTokenKind::Other,
                Token::Whitespace(Whitespace::SingleLineComment { .. }) => SemanticTokenKind::Comment,
                Token::Whitespace(Whitespace::MultiLineComment { .. }) => SemanticTokenKind::Comment,
                Token::Whitespace(_) => SemanticTokenKind::Other,
                Token::DoubleEq => SemanticTokenKind::Operator,
                Token::Eq => SemanticTokenKind::Operator,
                Token::Neq => SemanticTokenKind::Operator,
                Token::Lt => SemanticTokenKind::Operator,
                Token::Gt => SemanticTokenKind::Operator,
                Token::LtEq => SemanticTokenKind::Operator,
                Token::GtEq => SemanticTokenKind::Operator,
                Token::Spaceship => SemanticTokenKind::Operator,
                Token::Plus => SemanticTokenKind::Operator,
                Token::Minus => SemanticTokenKind::Operator,
                Token::Mul => SemanticTokenKind::Operator,
                Token::Div => SemanticTokenKind::Operator,
                Token::DuckIntDiv => SemanticTokenKind::Operator,
                Token::Mod => SemanticTokenKind::Operator,
                Token::StringConcat => SemanticTokenKind::Operator,
                Token::LParen => SemanticTokenKind::Other,
                Token::RParen => SemanticTokenKind::Other,
                Token::Period => SemanticTokenKind::Other,
                Token::Colon => SemanticTokenKind::Other,
                Token::DoubleColon => SemanticTokenKind::Operator,
                Token::SemiColon => SemanticTokenKind::Other,
                Token::Backslash => SemanticTokenKind::Operator,
                Token::LBracket => SemanticTokenKind::Other,
                Token::RBracket => SemanticTokenKind::Other,
                Token::Ampersand => SemanticTokenKind::Operator,
                Token::Pipe => SemanticTokenKind::Operator,
                Token::Caret => SemanticTokenKind::Operator,
                Token::LBrace => SemanticTokenKind::Other,
                Token::RBrace => SemanticTokenKind::Other,
                Token::RArrow => SemanticTokenKind::Operator,
                Token::Sharp => SemanticTokenKind::Operator,
                Token::Tilde => SemanticTokenKind::Operator,
                Token::TildeAsterisk => SemanticTokenKind::Operator,
                Token::ExclamationMarkTilde => SemanticTokenKind::Operator,
                Token::ExclamationMarkTildeAsterisk => SemanticTokenKind::Operator,
                Token::ShiftLeft => SemanticTokenKind::Operator,
                Token::ShiftRight => SemanticTokenKind::Operator,
                Token::ExclamationMark => SemanticTokenKind::Operator,
                Token::DoubleExclamationMark => SemanticTokenKind::Operator,
                Token::AtSign => SemanticTokenKind::Operator,
                Token::PGSquareRoot => SemanticTokenKind::Operator,
                Token::PGCubeRoot => SemanticTokenKind::Operator,
                Token::Placeholder(_string) => SemanticTokenKind::Operator,
                Token::Arrow => SemanticTokenKind::Operator,
                Token::LongArrow => SemanticTokenKind::Operator,
                Token::HashArrow => SemanticTokenKind::Operator,
                Token::HashLongArrow => SemanticTokenKind::Operator,
                Token::AtArrow => SemanticTokenKind::Operator,
                Token::ArrowAt => SemanticTokenKind::Operator,
                Token::HashMinus => SemanticTokenKind::Operator,
                Token::AtQuestion => SemanticTokenKind::Operator,
                Token::AtAt => SemanticTokenKind::Operator,
                Token::DuckAssignment => SemanticTokenKind::Operator,
                Token::Overlap => SemanticTokenKind::Operator,
            },
            start,
            end,
        });
    }
    tokens
}
