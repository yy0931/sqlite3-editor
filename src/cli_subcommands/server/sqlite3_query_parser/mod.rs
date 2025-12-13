//! Parses SQLite queries using sqlparser. Only this module should depend on sqlparser.

pub(super) mod paren_aware_token_scanner;
pub mod parse_cte;
pub mod split_statements;
pub mod token;
pub mod tokenize;
pub mod types;
