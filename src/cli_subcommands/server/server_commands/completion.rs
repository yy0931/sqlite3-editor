use crate::cli_error::CLIError;
use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableName;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
use crate::cli_subcommands::server::sqlite3_fns::table_names::table_names;
use crate::cli_subcommands::server::sqlite3_fns::table_schema::table_schema;
use crate::cli_subcommands::server::sqlite3_query_parser::paren_aware_token_scanner::SQLite3ParenAwareTokenScanner;
use crate::cli_subcommands::server::sqlite3_query_parser::paren_aware_token_scanner::SQLite3TokenScannerOutput;
use crate::cli_subcommands::server::sqlite3_query_parser::parse_cte::parse_cte;
use crate::cli_subcommands::server::sqlite3_query_parser::split_statements::split_sqlite_statements;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Keyword;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3KeywordLikeIdentifier;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Operator;
use crate::cli_subcommands::server::sqlite3_query_parser::token::SQLite3Token;
use crate::cli_subcommands::server::sqlite3_query_parser::types::WithZeroIndexedRange;
use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedLocation;
use crate::cli_subcommands::server::TruncateAll;
use rmp_serde::encode::write_named;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;

pub fn run(conn: &rusqlite::Connection, r: &mut File, w: &mut File) -> CLIErrorCode {
    // Handle the command
    if let Err(err) = rmp_serde::from_read(r).map(
        |CompletionCommandParams { sql, line, column }: CompletionCommandParams| -> std::result::Result<(), CLIError> {
            write_named(
                w,
                &complete(
                    conn,
                    &sql,
                    &ZeroIndexedLocation {
                        line: line.try_into().unwrap(),
                        column: column.try_into().unwrap(),
                    },
                ),
            )?;
            Ok(())
        },
    ) {
        w.flush().unwrap();
        w.truncate_all();
        write!(w, "{err:?}").unwrap();
        CLIErrorCode::OtherError
    } else {
        w.flush().unwrap();
        CLIErrorCode::Success
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(from = "(String, i64, i64)")]
struct CompletionCommandParams {
    sql: String,
    line: i64,
    column: i64,
}

impl From<(String, i64, i64)> for CompletionCommandParams {
    fn from(value: (String, i64, i64)) -> Self {
        Self {
            sql: value.0,
            line: value.1,
            column: value.2,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableCompletion {
    pub schema: Rc<String>,
    pub table: Rc<String>,
    #[serde(rename = "type")]
    pub type_: TableType,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ColumnCompletion {
    pub schema: Rc<String>,
    pub table: Rc<String>,
    pub column: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct Completions {
    pub table_names: HashSet<TableCompletion>,
    pub schema_names: HashSet<Rc<String>>,
    pub columns_in_tables_that_are_referenced_in_source: HashSet<ColumnCompletion>,
    pub cte_names: HashSet<String>,
    pub as_clauses: HashSet<String>,
    pub last_tokens: VecDeque<TokenType>,
    pub last_schema: Option<String>,
    pub last_table: Option<String>,
    pub last_create_trigger_table: Option<String>,
}

mod completions_ts {
    use super::ColumnCompletion;
    use super::TableCompletion;
    use super::TokenType;

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
        pub last_schema: Option<String>,
        pub last_table: Option<String>,
        pub last_create_trigger_table: Option<String>,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
enum AliasableToken {
    Ident(String),
    Other,
}

fn can_alias_follow(token: &SQLite3Token, previous: Option<AliasableToken>) -> Option<AliasableToken> {
    match token {
        // SELECT column alias
        //        ^token
        SQLite3Token::Identifier(value, _) => Some(AliasableToken::Ident(value.to_owned())),

        // SELECT column AS alias
        //               ^token
        SQLite3Token::Keyword(SQLite3Keyword::AS) => previous.or(Some(AliasableToken::Other)),

        // SELECT 123 alias
        //        ^token
        SQLite3Token::NumericLiteral(_)
        | SQLite3Token::StringLiteral(_)
        | SQLite3Token::BlobLiteral(_)
        | SQLite3Token::InvalidNumericLiteral
        | SQLite3Token::InvalidStringLiteral
        | SQLite3Token::RParen => Some(AliasableToken::Other),

        SQLite3Token::Whitespace(_) => previous,

        _ => None,
    }
}

fn is_token_before_cursor(token: &WithZeroIndexedRange<SQLite3Token>, position: &ZeroIndexedLocation) -> bool {
    match &token.value {
        SQLite3Token::Whitespace(_) => false,
        SQLite3Token::Period | SQLite3Token::LParen | SQLite3Token::RParen => {
            // pre_previous_token .|
            //                    ^ previous_token
            token.range.end <= *position
        }
        _ => {
            // pre_previous_token previous_token partial_token|
            token.range.end < *position
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, ts_rs::TS)]
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
    #[serde(rename = "=")]
    Equal,

    // Identifiers
    #[serde(rename = "<schema-name>")]
    SchemaIdent,
    #[serde(rename = "<table-name>")]
    TableIdent,

    // Literals
    #[serde(rename = "<literal>")]
    Value,

    // Group
    #[serde(rename = "(...)")]
    Group,

    // Other tokens
    Other,

    // Start of the statement
    #[serde(rename = "^")]
    StartOfStatement,
}

impl From<&SQLite3Token> for TokenType {
    fn from(value: &SQLite3Token) -> Self {
        match value {
            SQLite3Token::Keyword(SQLite3Keyword::PRAGMA) => TokenType::PRAGMA,
            SQLite3Token::Keyword(SQLite3Keyword::JOIN) => TokenType::JOIN,
            SQLite3Token::Keyword(SQLite3Keyword::FROM) => TokenType::FROM,
            SQLite3Token::Keyword(SQLite3Keyword::INSERT) => TokenType::INSERT,
            SQLite3Token::Keyword(SQLite3Keyword::INTO) => TokenType::INTO,
            SQLite3Token::Keyword(SQLite3Keyword::DROP) => TokenType::DROP,
            SQLite3Token::Keyword(SQLite3Keyword::TABLE) => TokenType::TABLE,
            SQLite3Token::Keyword(SQLite3Keyword::VIEW) => TokenType::VIEW,
            SQLite3Token::Keyword(SQLite3Keyword::AS) => TokenType::AS,
            SQLite3Token::Keyword(SQLite3Keyword::IF) => TokenType::IF,
            SQLite3Token::Keyword(SQLite3Keyword::NOT) => TokenType::NOTE,
            SQLite3Token::Keyword(SQLite3Keyword::EXISTS) => TokenType::EXISTS,
            SQLite3Token::Keyword(SQLite3Keyword::REPLACE) => TokenType::REPLACE,
            SQLite3Token::Keyword(SQLite3Keyword::OR) => TokenType::OR,
            SQLite3Token::Keyword(SQLite3Keyword::DELETE) => TokenType::DELETE,
            SQLite3Token::Keyword(SQLite3Keyword::ALTER) => TokenType::ALTER,
            SQLite3Token::Keyword(SQLite3Keyword::RENAME) => TokenType::RENAME,
            SQLite3Token::Keyword(SQLite3Keyword::COLUMN) => TokenType::COLUMN,
            SQLite3Token::Keyword(SQLite3Keyword::SELECT) => TokenType::SELECT,
            SQLite3Token::Keyword(SQLite3Keyword::WHERE) => TokenType::WHERE,
            SQLite3Token::Keyword(SQLite3Keyword::DISTINCT) => TokenType::DISTINCT,

            SQLite3Token::Identifier(_, Some(SQLite3KeywordLikeIdentifier::NEW)) => TokenType::NEW,
            SQLite3Token::Identifier(_, Some(SQLite3KeywordLikeIdentifier::OLD)) => TokenType::OLD,
            SQLite3Token::Identifier(_, Some(SQLite3KeywordLikeIdentifier::TRUE)) => TokenType::Value,
            SQLite3Token::Identifier(_, Some(SQLite3KeywordLikeIdentifier::FALSE)) => TokenType::Value,

            // TODO: NULL is not literal when in "column NOT NULL"
            SQLite3Token::LParen => TokenType::LParen,
            SQLite3Token::Period => TokenType::Period,
            SQLite3Token::Operator(SQLite3Operator::Eq) => TokenType::Equal,
            SQLite3Token::NumericLiteral(_)
            | SQLite3Token::StringLiteral(_)
            | SQLite3Token::BlobLiteral(_)
            | SQLite3Token::InvalidStringLiteral
            | SQLite3Token::InvalidNumericLiteral => TokenType::Value,
            _ => TokenType::Other,
        }
    }
}

pub fn complete(conn: &rusqlite::Connection, sql: &str, position: &ZeroIndexedLocation) -> Completions {
    // TODO: cache list_tables and table_schema if they are slow

    let stmt = split_sqlite_statements(sql)
        .unwrap_or_default()
        .0
        .into_iter()
        // include .end only for the last statement
        // `SELECT 1|; SELECT 2` -> | belongs to SELECT 1
        // `SELECT 1|          ` -> | belongs to SELECT 1
        // `SELECT 1;| SELECT 2` -> | belongs to SELECT 2
        .rev()
        .find(|stmt| stmt.all_text.range.start <= *position && *position <= stmt.all_text.range.end);

    let table_list = table_names(conn).unwrap_or_default().0;

    // table.name -> table
    let mut table_name_lowered_to_info = HashMap::<String, Vec<&TableName>>::new();
    for t in &table_list {
        table_name_lowered_to_info.entry(t.name.to_lowercase()).or_default().push(t);
    }

    let schema_names = table_list.iter().map(|t| &t.database).cloned().collect::<HashSet<_>>();
    let schema_names_lowered = schema_names.iter().map(|s| s.to_lowercase()).collect::<HashSet<_>>();

    let mut referenced_tables = HashSet::new();

    let mut cte_names = HashSet::new();
    let mut as_clauses_lower = HashMap::</* lower case */ String, (/* original case */ String, AliasableToken)>::new();

    let mut last_token_before_position: Option<usize> = None;
    let mut last_tokens = VecDeque::<TokenType>::new();
    let mut last_schema = None;
    let mut last_table: Option<Option<String>> = None;
    let mut last_create_trigger_table: Option<String> = None;

    if let Some(stmt) = stmt {
        let mut expect_followed_by_alias: Option<AliasableToken> = None;
        let mut create_trigger = 0;

        for (i, token) in stmt.real_tokens.iter().enumerate() {
            // Update as_clauses_lower and referenced_tables
            {
                if let SQLite3Token::Identifier(value, _) = &token.value {
                    if let Some(target) = expect_followed_by_alias {
                        as_clauses_lower.insert(value.to_lowercase(), (value.clone(), target));
                        expect_followed_by_alias = None;
                    }

                    if let Some(t) = table_name_lowered_to_info.get(&value.to_lowercase()).and_then(|v| v.first()) {
                        referenced_tables.insert(&t.name);
                    }
                }

                expect_followed_by_alias = can_alias_follow(&token.value, expect_followed_by_alias);
            }

            // Update last_token_before_position
            if is_token_before_cursor(token, position) {
                last_token_before_position = Some(i);
            }

            // Update last_create_trigger_table
            // CREATE TRIGGER ... ON <table>
            match &token.value {
                SQLite3Token::Keyword(SQLite3Keyword::CREATE) => create_trigger = 1,
                SQLite3Token::Keyword(SQLite3Keyword::TRIGGER) if create_trigger == 1 => create_trigger = 2,
                SQLite3Token::Keyword(SQLite3Keyword::ON) if create_trigger == 2 => create_trigger = 3,
                SQLite3Token::Identifier(value, _) if create_trigger == 3 => {
                    create_trigger = 0;
                    last_create_trigger_table = Some(value.to_owned());
                }
                SQLite3Token::Whitespace(_) => {}
                _ if create_trigger == 1 => create_trigger = 0,
                _ => {}
            }
        }

        if let Some(cte) = parse_cte(&stmt) {
            for entry in cte.entries {
                cte_names.insert(entry.ident.value);
            }
        }

        if let Some(last_token_before_position) = last_token_before_position {
            // Categorize tokens backward until taking an unsupported token or consuming 7 non-whitespace tokens
            for output in SQLite3ParenAwareTokenScanner::new(&stmt.real_tokens[0..=last_token_before_position]).rev() {
                match output {
                    SQLite3TokenScannerOutput::StartOfInput => {
                        last_tokens.push_front(TokenType::StartOfStatement);
                    }
                    SQLite3TokenScannerOutput::EndOfInput => {}
                    SQLite3TokenScannerOutput::Group(_) => {
                        last_tokens.push_front(TokenType::Group);
                    }
                    SQLite3TokenScannerOutput::SingleToken(token) => {
                        match &token.value {
                            SQLite3Token::Whitespace(_) => {}
                            // TEMP is a keyword but "TEMP" in "TEMP." is a schema name
                            SQLite3Token::Keyword(SQLite3Keyword::TEMP) if last_tokens.back() == Some(&TokenType::Period) => {
                                if last_schema.is_none() {
                                    last_schema = Some("temp".to_owned());
                                }
                                last_tokens.push_front(TokenType::SchemaIdent);
                            }
                            SQLite3Token::Identifier(value, None) => {
                                let value_lower = value.to_lowercase();
                                if schema_names_lowered.contains(&value_lower) {
                                    if last_schema.is_none() {
                                        last_schema = Some(value.to_owned());
                                    }
                                    last_tokens.push_front(TokenType::SchemaIdent);
                                } else {
                                    if last_table.is_none() {
                                        last_table = Some(if let Some((_, target)) = as_clauses_lower.get(&value_lower) {
                                            match target {
                                                AliasableToken::Ident(ident)
                                                    if table_name_lowered_to_info.contains_key(&ident.to_lowercase()) =>
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
                                    last_tokens.push_front(TokenType::TableIdent);
                                }
                            }
                            t => {
                                // TODO: NULL is not a literal when in "column NOT NULL"
                                last_tokens.push_front(t.into());
                            }
                        }
                    }
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
                if let Ok((Some(table_info), _)) = table_schema(conn, &table.database, &table.name) {
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
        as_clauses: as_clauses_lower.into_iter().map(|(_, (v, _))| v).collect::<HashSet<_>>(),
        last_tokens,
        last_schema,
        last_table: last_table.flatten(),
        last_create_trigger_table,
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use crate::cli_subcommands::server::server_commands::completion::complete;
    use crate::cli_subcommands::server::server_commands::completion::ColumnCompletion;
    use crate::cli_subcommands::server::server_commands::completion::Completions;
    use crate::cli_subcommands::server::server_commands::completion::TableCompletion;
    use crate::cli_subcommands::server::server_commands::completion::TokenType;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
    use crate::cli_subcommands::server::sqlite3_query_parser::types::ZeroIndexedLocation;

    fn hash_set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    fn loc(line: usize, column: usize) -> ZeroIndexedLocation {
        ZeroIndexedLocation { line, column }
    }

    fn setup() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE table_name(column_name)", ()).unwrap();
        conn
    }

    #[test]
    fn test_simple() {
        assert_eq!(
            complete(
                &setup(),
                "WITH cte_ident AS (SELECT 1 as as_ident1) SELECT 1 as as_ident2 FROM table_name",
                &loc(0, 10)
            ),
            Completions {
                table_names: HashSet::from([
                    TableCompletion {
                        schema: Rc::new("main".to_owned()),
                        table: Rc::new("sqlite_schema".to_owned()),
                        type_: TableType::Table,
                    },
                    TableCompletion {
                        schema: Rc::new("temp".to_owned()),
                        table: Rc::new("sqlite_temp_schema".to_owned()),
                        type_: TableType::Table,
                    },
                    TableCompletion {
                        schema: Rc::new("main".to_owned()),
                        table: Rc::new("table_name".to_owned()),
                        type_: TableType::Table,
                    },
                ]),
                schema_names: HashSet::from([Rc::new("main".to_owned()), Rc::new("temp".to_owned())]),
                columns_in_tables_that_are_referenced_in_source: HashSet::from([ColumnCompletion {
                    schema: Rc::new("main".to_owned()),
                    table: Rc::new("table_name".to_owned()),
                    column: "column_name".to_owned(),
                }]),
                cte_names: hash_set(&["cte_ident"]),
                as_clauses: hash_set(&["as_ident1", "as_ident2"]),
                last_tokens: VecDeque::from([TokenType::StartOfStatement, TokenType::Other]),
                last_schema: None,
                last_table: None,
                last_create_trigger_table: None,
            }
        );
    }

    #[test]
    fn test_quoted_ident() {
        let result = complete(
            &setup(),
            r#"
WITH
    [ident1] AS (SELECT 1),
    `ident2` AS (SELECT 1),
    "ident3" AS (SELECT 1)
SELECT
    1 AS [ident4],
    1 AS `ident5`,
    1 AS "ident6""#,
            &loc(3, 0),
        );
        assert_eq!(result.cte_names, hash_set(&["ident1", "ident2", "ident3"]));
        assert_eq!(result.as_clauses, hash_set(&["ident4", "ident5", "ident6"]));
    }

    #[test]
    fn test_as_clause_without_as() {
        let result = complete(
            &setup(),
            r#"
SELECT
    1 ident1,
    1 "ident2",
    +10 ident3,
    -10 ident4,
    'a' ident5,
    b + c ident6,
    (d < e) ident7,
    x'ff' ident8"#,
            &loc(3, 0),
        );
        assert_eq!(
            result.as_clauses,
            hash_set(&["ident1", "ident2", "ident3", "ident4", "ident5", "ident6", "ident7", "ident8"])
        );
    }

    #[test]
    fn test_json_encode() {
        dbg!(serde_json::to_string(&complete(
            &setup(),
            r#"
WITH
    [ident1] AS (SELECT 1),
    `ident2` AS (SELECT 1),
    "ident3" AS (SELECT 1)
SELECT
    1 AS [ident4],
    1 AS `ident5`,
    1 AS "ident6",
    1 AS (SELECT 1)"#,
            &loc(3, 0),
        ))
        .unwrap());
    }

    fn is_start_of_stmt(c: Completions) {
        assert_eq!(c.last_tokens, VecDeque::from([TokenType::StartOfStatement]));
    }

    fn is_not_start_of_stmt(c: Completions) {
        assert_ne!(c.last_tokens, VecDeque::from([TokenType::StartOfStatement]));
    }

    #[test]
    fn test_special_case_start_of_statement() {
        is_start_of_stmt(complete(&setup(), r#""#, &loc(0, 0)));
        is_start_of_stmt(complete(&setup(), r#"SELECT"#, &loc(0, 0)));
        is_start_of_stmt(complete(&setup(), r#"SELECT"#, &loc(0, 3)));
        is_start_of_stmt(complete(&setup(), r#"SELECT"#, &loc(0, 6)));
        is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 0)));
        is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 1)));
        is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 3)));
        is_start_of_stmt(complete(&setup(), r#"SEL"#, &loc(0, 3)));
        is_not_start_of_stmt(complete(&setup(), r#"SEL "#, &loc(0, 4)));
        is_not_start_of_stmt(complete(&setup(), r#"SEL a"#, &loc(0, 5)));
        is_not_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 8)));
        is_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 9)));
        is_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 10)));
        is_start_of_stmt(complete(&setup(), r#"SELECT a; SELECT"#, &loc(0, 11)));
    }

    fn is_after_join(mut c: Completions) {
        assert_eq!(c.last_tokens.pop_back(), Some(TokenType::JOIN));
    }

    fn is_not_after_join(mut c: Completions) {
        assert_ne!(c.last_tokens.pop_back(), Some(TokenType::JOIN));
    }

    #[test]
    fn test_special_case_after_join() {
        is_not_after_join(complete(&setup(), r#"SELECT * FROM t JOIN"#, &loc(0, 19)));
        is_not_after_join(complete(&setup(), r#"SELECT * FROM t JOIN"#, &loc(0, 20)));
        is_after_join(complete(&setup(), r#"SELECT * FROM t JOIN "#, &loc(0, 21)));
        is_after_join(complete(&setup(), r#"SELECT * FROM t JOIN a"#, &loc(0, 21)));
        is_after_join(complete(&setup(), r#"SELECT * FROM t JOIN a"#, &loc(0, 22)));
    }

    fn is_after_schema_period(mut c: Completions) {
        assert_eq!(
            (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
            (Some(TokenType::Period), Some(TokenType::SchemaIdent))
        );
    }

    fn is_not_after_schema_period(mut c: Completions) {
        assert_ne!(
            (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
            (Some(TokenType::Period), Some(TokenType::SchemaIdent))
        );
    }

    #[test]
    fn test_after_schema_period() {
        is_after_schema_period(complete(&setup(), r#"SELECT * FROM main."#, &loc(0, 19)));
        is_after_schema_period(complete(&setup(), r#"SELECT * FROM main. "#, &loc(0, 19)));
        is_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 19)));
        is_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 20)));
        is_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 21)));
        is_not_after_schema_period(complete(&setup(), r#"SELECT * FROM main.ab "#, &loc(0, 22)));
    }

    fn is_after_table_period(mut c: Completions) {
        assert_eq!(
            (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
            (Some(TokenType::Period), Some(TokenType::TableIdent))
        );
    }

    fn is_not_after_table_period(mut c: Completions) {
        assert_ne!(
            (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
            (Some(TokenType::Period), Some(TokenType::TableIdent))
        );
    }

    #[test]
    fn test_after_table_period() {
        is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd."#, &loc(0, 19)));
        is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd. "#, &loc(0, 19)));
        is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 19)));
        is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 20)));
        is_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 21)));
        is_not_after_table_period(complete(&setup(), r#"SELECT * FROM abcd.ab "#, &loc(0, 22)));
    }

    fn is_after_as_paren(mut c: Completions) {
        assert_eq!(
            (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
            (Some(TokenType::LParen), Some(TokenType::AS))
        );
    }

    fn is_not_after_as_paren(mut c: Completions) {
        assert_ne!(
            (c.last_tokens.pop_back(), c.last_tokens.pop_back()),
            (Some(TokenType::LParen), Some(TokenType::AS))
        );
    }

    #[test]
    fn test_after_as_paren() {
        is_not_after_as_paren(complete(&setup(), r#"WITH a AS ("#, &loc(0, 10)));
        is_after_as_paren(complete(&setup(), r#"WITH a AS ("#, &loc(0, 11)));
        is_after_as_paren(complete(&setup(), r#"WITH a AS (a"#, &loc(0, 12)));
    }

    #[test]
    fn test_temp_schema() {
        let mut result = complete(&setup(), r#"SELECT * FROM temp."#, &loc(0, 19));
        assert_eq!(result.last_tokens.pop_back(), Some(TokenType::Period));
        assert_eq!(result.last_tokens.pop_back(), Some(TokenType::SchemaIdent));
        assert_eq!(result.last_schema, Some("temp".to_owned()));
    }

    #[test]
    fn test_nocase() {
        assert_eq!(
            complete(&setup(), r#"SELECT "Table_name". FROM "Table_name""#, &loc(0, 20)).columns_in_tables_that_are_referenced_in_source,
            HashSet::from([ColumnCompletion {
                schema: Rc::new("main".to_owned()),
                table: Rc::new("table_name".to_owned()),
                column: "column_name".to_owned(),
            }])
        );
    }

    #[test]
    fn test_group() {
        assert_eq!(
            complete(&setup(), r#"INSERT INTO t (1, 2)"#, &loc(0, 20)).last_tokens,
            vec![
                TokenType::StartOfStatement,
                TokenType::INSERT,
                TokenType::INTO,
                TokenType::TableIdent,
                TokenType::Group
            ]
        );
    }

    #[test]
    fn test1() {
        let db = setup();
        for stmt in &[r#"CREATE TABLE t1(x);"#, r#"CREATE TABLE t2(y);"#] {
            db.execute(stmt, ()).unwrap();
        }
        let mut result = complete(&db, r#"SELECT * FROM t1 A, JOIN t2 B WHERE t1.x = t2."#, &loc(0, 46));
        assert_eq!(result.last_tokens.pop_back().unwrap(), TokenType::Period);
        assert_eq!(result.last_tokens.pop_back().unwrap(), TokenType::TableIdent);
        assert_eq!(result.last_schema, None);
        assert_eq!(result.last_table, Some("t2".to_owned()));
    }

    #[test]
    fn test_last_table() {
        let db = setup();
        let result = complete(&db, r#"SELECT a.b, c."#, &loc(0, 14));
        assert_eq!(result.last_table, Some("c".to_owned()));
    }

    #[test]
    fn test_last_schema() {
        let db = setup();
        let result = complete(&db, r#"SELECT * FROM temp.b, main."#, &loc(0, 27));
        assert_eq!(result.last_schema, Some("main".to_owned()));
    }

    #[test]
    fn test_last_create_trigger_table() {
        let db = setup();
        let result = complete(&db, r#"CREATE TRIGGER trigger1 BEFORE INSERT ON "table1" BEGIN"#, &loc(0, 46));
        assert_eq!(result.last_create_trigger_table, Some("table1".to_owned()));
    }

    #[test]
    fn test_pragma_encoding() {
        let db = setup();
        let result = complete(&db, r#"PRAGMA main.encoding = "#, &loc(0, 23));
        assert_eq!(
            result.last_tokens,
            [
                TokenType::StartOfStatement,
                TokenType::PRAGMA,
                TokenType::SchemaIdent,
                TokenType::Period,
                TokenType::TableIdent,
                TokenType::Equal
            ],
        );
        assert_eq!(result.last_table, Some("encoding".to_owned()));
    }
}
