use crate::semantic_highlight::{semantic_highlight, SemanticTokenKind};

#[test]
fn test_simple() {
    assert_eq!(
        semantic_highlight("SELECT 1;")
            .into_iter()
            .map(|t| t.kind)
            .collect::<Vec<_>>(),
        [
            SemanticTokenKind::Keyword, // "SELECT"
            SemanticTokenKind::Other,   // " "
            SemanticTokenKind::Number,  // "1"
            SemanticTokenKind::Other,   // ";"
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
