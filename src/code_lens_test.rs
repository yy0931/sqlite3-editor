use crate::{
    code_lens::{code_lens, CodeLens, CodeLensKind},
    tokenize::ZeroIndexedLocation,
};

#[test]
fn test_select() {
    assert_eq!(
        code_lens("SELECT 1; SELECT 2; VALUES(3);"),
        [
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 9),
                stmt_executed: "SELECT 1;".to_owned(),
            },
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 10),
                end: ZeroIndexedLocation::new(0, 19),
                stmt_executed: "SELECT 2;".to_owned(),
            },
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 20),
                end: ZeroIndexedLocation::new(0, 30),
                stmt_executed: "VALUES(3);".to_owned(),
            },
        ]
    );
}

#[test]
fn test_with_clause() {
    assert_eq!(
        code_lens("WITH a AS (SELECT 1) SELECT 2;"),
        [
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 5),
                end: ZeroIndexedLocation::new(0, 6),
                stmt_executed: "WITH a AS (SELECT 1) SELECT * FROM `a`".to_owned(),
            },
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 30),
                stmt_executed: "WITH a AS (SELECT 1) SELECT 2;".to_owned(),
            },
        ]
    );
}

#[test]
fn test_explain() {
    assert_eq!(code_lens("EXPLAIN SELECT 1;")[0].kind, CodeLensKind::Explain);
}

#[test]
fn test_other() {
    assert_eq!(
        code_lens("DROP TABLE t; ATTACH 'db' as db"),
        [
            CodeLens {
                kind: CodeLensKind::Other,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 13),
                stmt_executed: "DROP TABLE t;".to_owned(),
            },
            CodeLens {
                kind: CodeLensKind::Other,
                start: ZeroIndexedLocation::new(0, 14),
                end: ZeroIndexedLocation::new(0, 31),
                stmt_executed: "ATTACH 'db' as db".to_owned(),
            }
        ]
    );
}

#[test]
fn test_begin_end() {
    assert_eq!(
        code_lens("BEGIN; SELECT 1; SELECT 2; END;"),
        [CodeLens {
            kind: CodeLensKind::Other,
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 31),
            stmt_executed: "BEGIN; SELECT 1; SELECT 2; END;".to_owned(),
        }]
    );
}

#[test]
fn test_pragma() {
    assert_eq!(
        code_lens("PRAGMA analysis_limit;"),
        [CodeLens {
            kind: CodeLensKind::Other,
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 22),
            stmt_executed: "PRAGMA analysis_limit;".to_owned(),
        }]
    );
}

#[test]
fn test_vacuum() {
    assert_eq!(
        code_lens("VACUUM;"),
        [CodeLens {
            kind: CodeLensKind::Other,
            start: ZeroIndexedLocation::new(0, 0),
            end: ZeroIndexedLocation::new(0, 7),
            stmt_executed: "VACUUM;".to_owned(),
        }]
    );
}

#[test]
fn test_with_update() {
    assert_eq!(
        code_lens("WITH x AS (SELECT 1) UPDATE t SET a = 1;"),
        [
            CodeLens {
                kind: CodeLensKind::Select,
                start: ZeroIndexedLocation::new(0, 5),
                end: ZeroIndexedLocation::new(0, 6),
                stmt_executed: "WITH x AS (SELECT 1) SELECT * FROM `x`".to_owned(),
            },
            CodeLens {
                kind: CodeLensKind::Other,
                start: ZeroIndexedLocation::new(0, 0),
                end: ZeroIndexedLocation::new(0, 40),
                stmt_executed: "WITH x AS (SELECT 1) UPDATE t SET a = 1;".to_owned(),
            }
        ]
    );
}
