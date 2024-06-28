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
        tokenize_with_range_location(&SQLiteDialect {}, "SELECT   1;")
            .unwrap()
            .into_iter()
            .map(|t| format!("{:?}", t))
            .collect::<Vec<_>>(),
        [
            r#"{0:0-0:6 Word(Word { value: "SELECT", quote_style: None, keyword: SELECT })}"#,
            r#"{0:6-0:9 Whitespace(Space)}"#,
            r#"{0:9-0:10 Number("1", false)}"#,
            r#"{0:10-0:11 SemiColon}"#,
        ],
    );
}
