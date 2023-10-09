use serde::{Deserialize, Serialize};
use sqlparser::{
    dialect::SQLiteDialect,
    keywords::Keyword,
    tokenizer::{Token, Whitespace, Word},
};

use crate::{
    keywords::KEYWORDS_UNSUPPORTED_BY_SQLPARSER,
    tokenize::{tokenize_with_range_location, TokenWithRangeLocation, ZeroIndexedLocation},
};

/// Represents the kind of token highlighting.
#[derive(ts_rs::TS, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(ts_rs::TS, Debug, Clone, Serialize, Deserialize)]
#[ts(export)]
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
                // word
                Token::Word(w) => match w {
                    Word {
                        quote_style: None,
                        value,
                        keyword: Keyword::NoKeyword,
                    } if KEYWORDS_UNSUPPORTED_BY_SQLPARSER.contains(value.to_uppercase().as_str()) => {
                        SemanticTokenKind::Keyword
                    }
                    Word {
                        keyword: Keyword::NoKeyword,
                        ..
                    } => SemanticTokenKind::Variable,
                    _ => SemanticTokenKind::Keyword,
                },

                // number
                Token::Number(_, _) => SemanticTokenKind::Number,

                // string
                Token::SingleQuotedString(_)
                | Token::DollarQuotedString(_)
                | Token::SingleQuotedByteStringLiteral(_)
                | Token::DoubleQuotedByteStringLiteral(_)
                | Token::RawStringLiteral(_)
                | Token::NationalStringLiteral(_)
                | Token::EscapedStringLiteral(_)
                | Token::HexStringLiteral(_) => SemanticTokenKind::String,

                // variable
                Token::DoubleQuotedString(_) => SemanticTokenKind::Variable,

                // comment
                Token::Whitespace(Whitespace::SingleLineComment { .. })
                | Token::Whitespace(Whitespace::MultiLineComment { .. }) => SemanticTokenKind::Comment,

                // operator
                Token::DoubleEq
                | Token::Eq
                | Token::Neq
                | Token::Lt
                | Token::Gt
                | Token::LtEq
                | Token::GtEq
                | Token::Spaceship
                | Token::Plus
                | Token::Minus
                | Token::Mul
                | Token::Div
                | Token::DuckIntDiv
                | Token::Mod
                | Token::StringConcat
                | Token::DoubleColon
                | Token::Backslash
                | Token::Ampersand
                | Token::Pipe
                | Token::Caret
                | Token::RArrow
                | Token::Sharp
                | Token::Tilde
                | Token::TildeAsterisk
                | Token::ExclamationMarkTilde
                | Token::ExclamationMarkTildeAsterisk
                | Token::ShiftLeft
                | Token::ShiftRight
                | Token::ExclamationMark
                | Token::DoubleExclamationMark
                | Token::AtSign
                | Token::PGSquareRoot
                | Token::PGCubeRoot
                | Token::Placeholder(_)
                | Token::Arrow
                | Token::LongArrow
                | Token::HashArrow
                | Token::HashLongArrow
                | Token::AtArrow
                | Token::ArrowAt
                | Token::HashMinus
                | Token::AtQuestion
                | Token::AtAt
                | Token::DuckAssignment
                | Token::Overlap => SemanticTokenKind::Operator,

                // other
                Token::EOF
                | Token::Char(_)
                | Token::Comma
                | Token::Whitespace(_)
                | Token::LParen
                | Token::RParen
                | Token::Period
                | Token::Colon
                | Token::SemiColon
                | Token::LBracket
                | Token::RBracket
                | Token::LBrace
                | Token::RBrace => SemanticTokenKind::Other,
            },
            start,
            end,
        });
    }
    tokens
}
