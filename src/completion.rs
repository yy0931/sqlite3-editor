use std::{
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};

use crate::{
    keywords::KEYWORDS_UNSUPPORTED_BY_SQLPARSER,
    parse_cte::parse_cte,
    split_statements::split_sqlite_statements,
    sqlite3_driver::{SQLite3Driver, Table, TableType},
    tokenize::{TokenWithRangeLocation, ZeroIndexedLocation},
};
use serde::{Deserialize, Serialize};
use sqlparser::{
    keywords::Keyword,
    tokenizer::{Token, Word},
};

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[ts(export)]
pub struct TableCompletion {
    pub schema: Rc<String>,
    pub table: Rc<String>,
    #[serde(rename = "type")]
    pub type_: TableType,
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[ts(export)]
pub struct ColumnCompletion {
    pub schema: Rc<String>,
    pub table: Rc<String>,
    pub column: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Completions {
    pub table_names: HashSet<TableCompletion>,
    pub schema_names: HashSet<Rc<String>>,
    pub columns_in_tables_that_are_referenced_in_source: HashSet<ColumnCompletion>,
    pub cte_names: HashSet<String>,
    pub as_clauses: HashSet<String>,
    pub last_tokens: VecDeque<TokenType>,
    pub last_schema_period: Option<String>,
    pub last_table_period: Option<String>,
    pub last_create_trigger_table: Option<String>,
}

mod completions_ts {
    use super::{ColumnCompletion, TableCompletion, TokenType};

    #[derive(ts_rs::TS)]
    #[ts(export)]
    #[allow(unused)]
    struct Completions {
        pub table_names: Vec<TableCompletion>,
        pub schema_names: Vec<String>,
        pub columns_in_tables_that_are_referenced_in_source: Vec<ColumnCompletion>,
        pub cte_names: Vec<String>,
        pub as_clauses: Vec<String>,
        pub last_tokens: Vec<TokenType>,
        pub last_schema_period: Option<String>,
        pub last_table_period: Option<String>,
        pub last_create_trigger_table: Option<String>,
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
enum AliasableToken {
    Ident(String),
    Other,
}

fn can_alias_follow(token: &Token, previous: Option<AliasableToken>) -> Option<AliasableToken> {
    match token {
        // <name> <alias>
        Token::Word(Word {
            keyword: Keyword::NoKeyword,
            value,
            ..
        }) if !KEYWORDS_UNSUPPORTED_BY_SQLPARSER.contains(&value.to_uppercase().as_str()) => {
            Some(AliasableToken::Ident(value.to_owned()))
        }

        // ... AS <alias>
        Token::Word(Word {
            keyword: Keyword::AS, ..
        }) => previous.or(Some(AliasableToken::Other)),

        // <literal> <alias>
        Token::Number(_, _)
        | Token::SingleQuotedString(_)
        | Token::DoubleQuotedString(_)
        | Token::DollarQuotedString(_)
        | Token::SingleQuotedByteStringLiteral(_)
        | Token::DoubleQuotedByteStringLiteral(_)
        | Token::RawStringLiteral(_)
        | Token::NationalStringLiteral(_)
        | Token::EscapedStringLiteral(_)
        | Token::HexStringLiteral(_)
        | Token::RParen
        | Token::RBracket
        | Token::RBrace => Some(AliasableToken::Other),

        Token::Whitespace(_) => previous,

        Token::EOF
        | Token::Word(_)
        | Token::Char(_)
        | Token::Comma
        | Token::DoubleEq
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
        | Token::LParen
        | Token::Period
        | Token::Colon
        | Token::DoubleColon
        | Token::DuckAssignment
        | Token::SemiColon
        | Token::Backslash
        | Token::LBracket
        | Token::Ampersand
        | Token::Pipe
        | Token::Caret
        | Token::LBrace
        | Token::RArrow
        | Token::Sharp
        | Token::Tilde
        | Token::TildeAsterisk
        | Token::ExclamationMarkTilde
        | Token::ExclamationMarkTildeAsterisk
        | Token::ShiftLeft
        | Token::ShiftRight
        | Token::Overlap
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
        | Token::AtAt => None,
    }
}

fn is_token_before_cursor(token: &TokenWithRangeLocation, position: &ZeroIndexedLocation) -> bool {
    match &token.token {
        Token::Whitespace(_) => false,
        Token::Period | Token::LParen | Token::RParen => {
            // pre_previous_token .|
            //                    ^ previous_token
            token.end <= *position
        }
        _ => {
            // pre_previous_token previous_token partial_token|
            token.end < *position
        }
    }
}

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[ts(export)]
#[allow(clippy::upper_case_acronyms)]
pub enum TokenType {
    // Keywords
    JOIN,
    FROM,
    INSERT,
    INTO,
    DROP,
    TABLE,
    VIEW,
    PRAGMA,
    AS,
    IF,
    NOTE,
    EXISTS,
    REPLACE,
    OR,
    DELETE,
    ALTER,
    RENAME,
    COLUMN,
    SELECT,
    WHERE,
    DISTINCT,
    NEW,
    OLD,

    // Operators
    #[serde(rename = "(")]
    LParen,
    #[serde(rename = ".")]
    Period,

    // Identifiers
    #[serde(rename = "<schema-name>")]
    SchemaIdent,
    #[serde(rename = "<table-name>")]
    TableIdent,

    // Literals
    #[serde(rename = "<literal>")]
    Literal,

    // Group
    #[serde(rename = "(...)")]
    Group,

    // Other tokens
    Other,

    // Start of the statement
    StartOfStatement,
}

pub fn complete(conn: &SQLite3Driver, sql: &str, position: &ZeroIndexedLocation) -> Completions {
    // TODO: cache list_tables and table_schema if they are slow

    let stmt = split_sqlite_statements(sql)
        .unwrap_or_default()
        .into_iter()
        // include .end only for the last statement
        // `SELECT 1|; SELECT 2` -> | belongs to SELECT 1
        // `SELECT 1|          ` -> | belongs to SELECT 1
        // `SELECT 1;| SELECT 2` -> | belongs to SELECT 2
        .rev()
        .find(|stmt| stmt.start <= *position && *position <= stmt.end);

    let table_list = conn.list_tables(true).unwrap_or_default().0;

    // table.name -> table
    let mut table_name_lowered_to_info = HashMap::<String, Vec<&Table>>::new();
    for t in &table_list {
        table_name_lowered_to_info
            .entry(t.name.to_lowercase())
            .or_default()
            .push(t);
    }

    let schema_names = table_list
        .iter()
        .map(|t| Rc::clone(&t.database))
        .collect::<HashSet<_>>();
    let schema_names_lowered = schema_names.iter().map(|s| s.to_lowercase()).collect::<HashSet<_>>();

    let mut referenced_tables = HashSet::new();

    let mut cte_names = HashSet::new();
    let mut as_clauses_lower = HashMap::</* lower case */ String, (/* original case */ String, AliasableToken)>::new();

    let mut last_token_before_position: Option<usize> = None;
    let mut last_tokens = VecDeque::<TokenType>::new();
    let mut last_schema_period = None;
    let mut last_table_period: Option<Option<String>> = None;
    let mut last_create_trigger_table: Option<String> = None;

    if let Some(stmt) = stmt {
        let mut expect_followed_by_alias: Option<AliasableToken> = None;
        let mut create_trigger = 0;

        for (i, token) in stmt.real_tokens.iter().enumerate() {
            // Update as_clauses_lower and referenced_tables
            {
                match &token.token {
                    Token::Word(Word {
                        keyword: Keyword::NoKeyword,
                        value,
                        ..
                    }) if !KEYWORDS_UNSUPPORTED_BY_SQLPARSER.contains(&value.to_uppercase().as_str()) => {
                        if let Some(target) = expect_followed_by_alias {
                            as_clauses_lower.insert(value.to_lowercase(), (value.clone(), target));
                            expect_followed_by_alias = None;
                        }

                        if let Some(t) = table_name_lowered_to_info
                            .get(&value.to_lowercase())
                            .and_then(|v| v.first())
                        {
                            referenced_tables.insert(&t.name);
                        }
                    }
                    _ => {}
                }

                expect_followed_by_alias = can_alias_follow(&token.token, expect_followed_by_alias);
            }

            // Update last_token_before_position
            if is_token_before_cursor(token, position) {
                last_token_before_position = Some(i);
            }

            // Update last_create_trigger_table
            // CREATE TRIGGER ... ON <table>
            match &token.token {
                Token::Word(Word {
                    keyword: Keyword::CREATE,
                    ..
                }) => create_trigger = 1,
                Token::Word(Word {
                    keyword: Keyword::TRIGGER,
                    ..
                }) if create_trigger == 1 => create_trigger = 2,
                Token::Word(Word {
                    keyword: Keyword::ON, ..
                }) if create_trigger == 2 => create_trigger = 3,
                Token::Word(Word {
                    keyword: Keyword::NoKeyword,
                    value,
                    ..
                }) if create_trigger == 3
                    && !KEYWORDS_UNSUPPORTED_BY_SQLPARSER.contains(&value.to_uppercase().as_str()) =>
                {
                    create_trigger = 0;
                    last_create_trigger_table = Some(value.to_owned());
                }
                Token::Whitespace(_) => {}
                _ if create_trigger == 1 => create_trigger = 0,
                _ => {}
            }
        }

        if let Some(cte) = parse_cte(&stmt) {
            for entry in cte.entries {
                cte_names.insert(entry.ident_text);
            }
        }

        if let Some(last_token_before_position) = last_token_before_position {
            // Categorize tokens backward until taking an unsupported token or consuming 7 non-whitespace tokens
            let mut depth = 0;
            for i in (0..=last_token_before_position).rev() {
                let token = &stmt.real_tokens[i];

                if depth > 0 {
                    match &token.token {
                        Token::RParen => {
                            depth += 1;
                        }
                        Token::LParen => {
                            depth -= 1;
                        }
                        _ => {}
                    }
                    continue;
                }

                match &token.token {
                    Token::Whitespace(_) => {}
                    _ => {
                        last_tokens.push_front(match &token.token {
                            Token::Word(Word {
                                keyword: Keyword::NoKeyword,
                                quote_style: None,
                                value,
                            }) if value.to_uppercase() == "PRAGMA" => TokenType::PRAGMA,
                            Token::Word(Word {
                                keyword: Keyword::NoKeyword,
                                value,
                                ..
                            }) if !KEYWORDS_UNSUPPORTED_BY_SQLPARSER.contains(value.to_uppercase().as_str()) => {
                                let value_lower = value.to_lowercase();
                                if schema_names_lowered.contains(&value_lower) {
                                    last_schema_period = Some(value.to_owned());
                                    TokenType::SchemaIdent
                                } else {
                                    if last_table_period.is_none() {
                                        last_table_period =
                                            Some(if let Some((_, target)) = as_clauses_lower.get(&value_lower) {
                                                match target {
                                                    AliasableToken::Ident(ident)
                                                        if table_name_lowered_to_info
                                                            .contains_key(&ident.to_lowercase()) =>
                                                    {
                                                        Some(ident.to_owned())
                                                    }
                                                    _ => None,
                                                }
                                            } else if cte_names.iter().any(|c| c.to_lowercase() == value_lower) {
                                                None
                                            } else {
                                                Some(value.to_owned())
                                            });
                                    }
                                    TokenType::TableIdent
                                }
                            }
                            Token::Word(Word { keyword, .. }) => match keyword {
                                Keyword::JOIN => TokenType::JOIN,
                                Keyword::FROM => TokenType::FROM,
                                Keyword::INSERT => TokenType::INSERT,
                                Keyword::INTO => TokenType::INTO,
                                Keyword::DROP => TokenType::DROP,
                                Keyword::TABLE => TokenType::TABLE,
                                Keyword::VIEW => TokenType::VIEW,
                                Keyword::AS => TokenType::AS,
                                Keyword::IF => TokenType::IF,
                                Keyword::NOT => TokenType::NOTE,
                                Keyword::EXISTS => TokenType::EXISTS,
                                Keyword::REPLACE => TokenType::REPLACE,
                                Keyword::OR => TokenType::OR,
                                Keyword::DELETE => TokenType::DELETE,
                                Keyword::ALTER => TokenType::ALTER,
                                Keyword::RENAME => TokenType::RENAME,
                                Keyword::COLUMN => TokenType::COLUMN,
                                Keyword::SELECT => TokenType::SELECT,
                                Keyword::WHERE => TokenType::WHERE,
                                Keyword::DISTINCT => TokenType::DISTINCT,
                                Keyword::NEW => TokenType::NEW,
                                Keyword::OLD => TokenType::OLD,

                                // TODO: NULL is not literal when in "column NOT NULL"
                                Keyword::TRUE | Keyword::FALSE => TokenType::Literal,

                                // TEMP is a keyword but "TEMP" in "TEMP." is a schema name
                                Keyword::TEMP if last_tokens.back() == Some(&TokenType::Period) => {
                                    last_schema_period = Some("temp".to_owned());
                                    TokenType::SchemaIdent
                                }

                                _ => TokenType::Other,
                            },
                            Token::LParen => TokenType::LParen,
                            Token::RParen => {
                                depth += 1;
                                TokenType::Group
                            }
                            Token::Period => TokenType::Period,
                            Token::Number(_, _)
                            | Token::SingleQuotedString(_)
                            | Token::DoubleQuotedString(_)
                            | Token::DollarQuotedString(_)
                            | Token::SingleQuotedByteStringLiteral(_)
                            | Token::DoubleQuotedByteStringLiteral(_)
                            | Token::RawStringLiteral(_)
                            | Token::NationalStringLiteral(_)
                            | Token::EscapedStringLiteral(_)
                            | Token::HexStringLiteral(_) => TokenType::Literal,
                            _ => TokenType::Other,
                        });
                    }
                }
                if i == 0 {
                    last_tokens.push_front(TokenType::StartOfStatement);
                }
                if last_tokens.len() >= 7 {
                    break;
                }
            }
        } else {
            last_tokens.push_front(TokenType::StartOfStatement);
        }
    } else {
        last_tokens.push_front(TokenType::StartOfStatement);
    }

    let mut columns_in_tables_that_are_referenced_in_source = HashSet::new();

    for table in referenced_tables {
        if let Some(tables) = table_name_lowered_to_info.get(&table.to_lowercase()) {
            for table in tables {
                if let Ok((Some(table_info), _)) = conn.table_schema(&table.database, &table.name) {
                    for c in table_info.columns {
                        columns_in_tables_that_are_referenced_in_source.insert(ColumnCompletion {
                            schema: Rc::clone(&table.database),
                            table: Rc::clone(&table.name),
                            column: c.name,
                        });
                    }
                }
            }
        }
    }

    Completions {
        table_names: table_list
            .into_iter()
            .map(|t| TableCompletion {
                schema: Rc::clone(&t.database),
                table: Rc::clone(&t.name),
                type_: t.type_,
            })
            .collect::<HashSet<_>>(),
        schema_names,
        columns_in_tables_that_are_referenced_in_source,
        cte_names,
        as_clauses: as_clauses_lower
            .into_iter()
            .map(|(_, (v, _))| v)
            .collect::<HashSet<_>>(),
        last_tokens,
        last_schema_period,
        last_table_period: last_table_period.flatten(),
        last_create_trigger_table,
    }
}
