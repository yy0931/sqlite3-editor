//! This module defines functions for retrieving data from a rusqlite connection (`rusqlite::Connection` or `rusqlite::ffi::sqlite3`).
//! This module must not use `SQLite3Connection`.

pub mod assert_readonly_query;
pub mod column_origin;
pub mod get_foreign_keys;
pub mod is_rowid;
pub mod list_references;
pub mod list_tables;
pub mod query_schema;
pub mod schema_types;
pub mod select_all;
pub mod table_names;
pub mod table_schema;
