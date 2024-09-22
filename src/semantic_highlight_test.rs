use crate::semantic_highlight::{semantic_highlight, SemanticTokenKind};

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
        semantic_highlight("SELECT 0x'ff'")
            .into_iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>(),
        [
            SemanticTokenKind::Keyword, // "SELECT"
            SemanticTokenKind::Other,   // " "
            SemanticTokenKind::String,  // "0x"
            SemanticTokenKind::String,  // "'ff'"
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
