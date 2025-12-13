#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SQLite3Token {
    NumericLiteral(String),
    StringLiteral(String),
    BlobLiteral(String),
    Keyword(SQLite3Keyword),
    Identifier(String, Option<SQLite3KeywordLikeIdentifier>),
    Placeholder(String, SQLite3PlaceholderKind),
    Operator(SQLite3Operator),

    Comma,
    LParen,
    RParen,
    Period,
    SemiColon,
    Whitespace(/* is_comment */ bool),

    // tokens that do not exist in SQLite
    InvalidNumericLiteral,
    InvalidStringLiteral,
    InvalidOperator,
    InvalidToken,
}

/// Represents a category of keyword-like identifiers.
///
/// In SQLite3, certain names can function both as identifiers and as keywords when unquoted.
/// For example, both `CREATE TABLE TRUE(x);` and `SELECT TRUE;` are valid statements.
///
/// This enum classifies such identifiers.
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Eq, PartialEq, Hash, Copy, strum_macros::EnumString, strum_macros::EnumIter, strum_macros::AsRefStr)]
pub enum SQLite3KeywordLikeIdentifier {
    TRUE,
    FALSE,
    NEW,
    OLD,
    BLOB,
    INTEGER,
    REAL,
    TEXT,
    ROWID,
    EACH,
    END,
    STORED,
    STRICT,
}

impl SQLite3Token {
    pub fn is_whitespace(&self) -> bool {
        matches!(self, Self::Whitespace(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SQLite3PlaceholderKind {
    /// ?
    Question,
    /// ?NNN
    QuestionNumber,
    /// :VVV
    ColonName,
    /// @VVV
    AtName,
    /// $VVV
    DollarName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SQLite3Operator {
    DoubleEq,
    Eq,
    Neq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    StringConcat,
    Ampersand,
    Pipe,
    Tilde,
    ShiftLeft,
    ShiftRight,
    Arrow,
    LongArrow,
}

#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Eq, PartialEq, Hash, Copy, strum_macros::EnumString, strum_macros::EnumIter, strum_macros::AsRefStr)]
pub enum SQLite3Keyword {
    ABORT,
    ACTION,
    ADD,
    AFTER,
    ALL,
    ALTER,
    ALWAYS,
    ANALYZE,
    AND,
    AS,
    ASC,
    ATTACH,
    AUTOINCREMENT,
    BEFORE,
    BEGIN,
    BETWEEN,
    BY,
    CASCADE,
    CASE,
    CAST,
    CHECK,
    COLLATE,
    COLUMN,
    COMMIT,
    CONFLICT,
    CONSTRAINT,
    CREATE,
    CROSS,
    CURRENT,
    CURRENT_DATE,
    CURRENT_TIME,
    CURRENT_TIMESTAMP,
    DATABASE,
    DEFAULT,
    DEFERRED,
    DEFERRABLE,
    DELETE,
    DESC,
    DETACH,
    DISTINCT,
    DO,
    DROP,
    END,
    EACH,
    ELSE,
    ESCAPE,
    EXCEPT,
    EXCLUSIVE,
    EXCLUDE,
    EXISTS,
    EXPLAIN,
    FAIL,
    FILTER,
    FIRST,
    FOLLOWING,
    FOR,
    FOREIGN,
    FROM,
    FULL,
    GENERATED,
    GLOB,
    GROUP,
    GROUPS,
    HAVING,
    IF,
    IGNORE,
    IMMEDIATE,
    IN,
    INDEX,
    INDEXED,
    INITIALLY,
    INNER,
    INSERT,
    INSTEAD,
    INTERSECT,
    INTO,
    IS,
    ISNULL,
    JOIN,
    KEY,
    LAST,
    LEFT,
    LIKE,
    LIMIT,
    MATCH,
    MATERIALIZED,
    NATURAL,
    NO,
    NOT,
    NOTHING,
    NOTNULL,
    NULL,
    NULLS,
    OF,
    OFFSET,
    ON,
    OR,
    ORDER,
    OTHERS,
    OUTER,
    OVER,
    PARTITION,
    PLAN,
    PRAGMA,
    PRECEDING,
    PRIMARY,
    QUERY,
    RAISE,
    RANGE,
    RECURSIVE,
    REFERENCES,
    REGEXP,
    REINDEX,
    RELEASE,
    RENAME,
    REPLACE,
    RESTRICT,
    RETURNING,
    RIGHT,
    ROLLBACK,
    ROW,
    ROWS,
    SAVEPOINT,
    SELECT,
    SET,
    TABLE,
    TEMP,
    TEMPORARY,
    THEN,
    TIES,
    TO,
    TRANSACTION,
    TRIGGER,
    UNBOUNDED,
    UNION,
    UNIQUE,
    UPDATE,
    USING,
    VACUUM,
    VALUES,
    VIEW,
    VIRTUAL,
    WHEN,
    WHERE,
    WINDOW,
    WITH,
    WITHIN,
    WITHOUT,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use strum::IntoEnumIterator;

    use super::SQLite3KeywordLikeIdentifier;

    use super::SQLite3Keyword;

    #[test]
    fn test_keyword_like_identifier_from_str() {
        assert_eq!(Ok(SQLite3KeywordLikeIdentifier::NEW), SQLite3KeywordLikeIdentifier::from_str("NEW"));
        assert_eq!(
            Ok(SQLite3KeywordLikeIdentifier::STRICT),
            SQLite3KeywordLikeIdentifier::from_str("STRICT")
        );
    }

    #[test]
    fn test_keyword_from_str() {
        assert_eq!(Ok(SQLite3Keyword::ABORT), SQLite3Keyword::from_str("ABORT"));
        assert_eq!(Ok(SQLite3Keyword::CURRENT_DATE), SQLite3Keyword::from_str("CURRENT_DATE"));
        assert_eq!(Ok(SQLite3Keyword::WITHOUT), SQLite3Keyword::from_str("WITHOUT"));

        assert_eq!(Err(strum::ParseError::VariantNotFound), SQLite3Keyword::from_str("abort"));
        assert_eq!(Err(strum::ParseError::VariantNotFound), SQLite3Keyword::from_str("current_date"));

        assert_eq!(Err(strum::ParseError::VariantNotFound), SQLite3Keyword::from_str(" ABORT"));
        assert_eq!(Err(strum::ParseError::VariantNotFound), SQLite3Keyword::from_str("ABORT "));
    }

    #[test]
    fn test_keyword_iter() {
        assert!(SQLite3Keyword::iter().len() >= 148);
        assert_eq!(SQLite3Keyword::iter().next().unwrap(), SQLite3Keyword::ABORT);
        assert_eq!(SQLite3Keyword::iter().next_back().unwrap(), SQLite3Keyword::WITHOUT);
    }

    #[test]
    fn test_keyword_as_ref() {
        assert_eq!(format!("{}", SQLite3Keyword::ABORT.as_ref()), "ABORT");
        assert_eq!(format!("{}", SQLite3Keyword::WITHOUT.as_ref()), "WITHOUT");
    }
}
