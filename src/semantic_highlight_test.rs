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
fn test_tokenizer_error() {
    assert_eq!(
        semantic_highlight("'aa")
            .into_iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>(),
        []
    );
}
