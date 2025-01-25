use std::collections::HashSet;

use once_cell::sync::Lazy;

pub static START_OF_STATEMENT_KEYWORDS_UNSUPPORTED_BY_SQLPARSER: Lazy<HashSet<&'static str>> =
    Lazy::new(|| HashSet::from(["VACUUM", "ATTACH", "DETACH", "PRAGMA", "REINDEX"]));
pub static KEYWORDS_UNSUPPORTED_BY_SQLPARSER: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "AFTER",
        "ATTACH",
        "BEFORE",
        "DEFERRABLE",
        "DEFERRED",
        "DETACH",
        "EXCLUSIVE",
        "GLOB",
        "IMMEDIATE",
        "INDEXED",
        "INITIALLY",
        "INSTEAD",
        "ISNULL",
        "NOTNULL",
        "OTHERS",
        "PLAN",
        "PRAGMA",
        "RAISE",
        "REGEXP",
        "REINDEX",
    ])
});
