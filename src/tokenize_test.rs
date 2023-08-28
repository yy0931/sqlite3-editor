use sqlparser::dialect::SQLiteDialect;

use crate::tokenize::tokenize_with_range_location;

#[test]
fn test_tokenize() {
    // Tokenize "CREATE TABLE t(c)"
    assert_eq!(
        tokenize_with_range_location(&SQLiteDialect {}, "CREATE TABLE t(c)")
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{<line 0, column 0>-<line 0, column 6>: Word(Word { value: "CREATE", quote_style: None, keyword: CREATE })}"#,
            r#"{<line 0, column 6>-<line 0, column 7>: Whitespace(Space)}"#,
            r#"{<line 0, column 7>-<line 0, column 12>: Word(Word { value: "TABLE", quote_style: None, keyword: TABLE })}"#,
            r#"{<line 0, column 12>-<line 0, column 13>: Whitespace(Space)}"#,
            r#"{<line 0, column 13>-<line 0, column 14>: Word(Word { value: "t", quote_style: None, keyword: NoKeyword })}"#,
            r#"{<line 0, column 14>-<line 0, column 15>: LParen}"#,
            r#"{<line 0, column 15>-<line 0, column 16>: Word(Word { value: "c", quote_style: None, keyword: NoKeyword })}"#,
            r#"{<line 0, column 16>-<line 0, column 17>: RParen}"#,
        ],
    );
}

#[test]
fn test_merge_whitespace() {
    // Test that "   " is tokenized into a single token
    assert_eq!(
        tokenize_with_range_location(&SQLiteDialect {}, "SELECT   1;")
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{<line 0, column 0>-<line 0, column 6>: Word(Word { value: "SELECT", quote_style: None, keyword: SELECT })}"#,
            r#"{<line 0, column 6>-<line 0, column 9>: Whitespace(Space)}"#,
            r#"{<line 0, column 9>-<line 0, column 10>: Number("1", false)}"#,
            r#"{<line 0, column 10>-<line 0, column 11>: SemiColon}"#,
        ],
    );
}
