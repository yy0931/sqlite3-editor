use std::collections::HashSet;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref START_OF_STATEMENT_KEYWORDS_UNSUPPORTED_BY_SQLPARSER: HashSet<&'static str> =
        HashSet::from(["VACUUM", "ATTACH", "DETACH", "PRAGMA", "REINDEX"]);
    pub static ref KEYWORDS_UNSUPPORTED_BY_SQLPARSER: HashSet<&'static str> = HashSet::from([
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
        "VACUUM",
    ]);
}
