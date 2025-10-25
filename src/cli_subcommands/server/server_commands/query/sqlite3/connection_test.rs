use std::io::Cursor;
use std::vec;

use crate::cli_subcommands::server::server_commands::query::request_type::QueryMode;
use crate::cli_subcommands::server::server_commands::query::request_type::QueryOptions;
use crate::cli_subcommands::server::server_commands::query::sqlite3::connection::ExecMode;
use crate::cli_subcommands::server::server_commands::query::sqlite3::connection::SQLite3Connection;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::cli_value::CLIValue;
use crate::msgpack::decode_msgpack_into_json;
use crate::utf8_extractor::InvalidUTF8;

fn convert_msgpack_to_json(input: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = vec![];
    serde_transcode::transcode(
        &mut rmp_serde::Deserializer::from_read_ref(input),
        &mut serde_json::Serializer::new(&mut buf),
    )
    .unwrap();
    Ok(String::from_utf8_lossy(&buf).to_string())
}

#[test]
fn test_execute_params() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();

    let params: Vec<CLIValue> = vec![CLIValue::Nil, "a".into(), 123.into(), 1.23.into(), vec![1, 2, 3].into()];
    let mut warnings = vec![];
    assert_eq!(
        decode_msgpack_into_json(
            db._execute(
                "SELECT ? as a, ? as b, ? as c, ? as d, ? as e",
                &params,
                ExecMode::ReadWrite,
                QueryOptions::default(),
                &mut warnings,
            )
            .unwrap()
        ),
        r#"{"a":[null],"b":["a"],"c":[123],"d":[1.23],"e":[[1,2,3]]}"#
    );
    assert_eq!(warnings, vec![]);
}

#[test]
fn test_values() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    assert_eq!(
        convert_msgpack_to_json(
            &db._execute(
                r#"
WITH
    temp_table (column1, column2) AS (VALUES (1, 2), (3, 4))
SELECT * FROM temp_table;
"#,
                &[],
                ExecMode::ReadWrite,
                QueryOptions::default(),
                &mut vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        "{\"column1\":[1,3],\"column2\":[2,4]}"
    );
}

#[test]
fn test_database_label() {
    let db = SQLite3Connection::connect(":memory:", false).unwrap();
    assert_ne!(db.database_label, "");
}

#[test]
fn test_query_error() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    let mut w = Cursor::new(Vec::<u8>::new());
    assert_eq!(
        format!(
            "{}",
            db.handle(
                &mut w,
                "SELECT * FROM non_existent",
                &[],
                QueryMode::ReadOnly,
                QueryOptions::default(),
            )
            .unwrap_err()
        ),
        "no such table: non_existent\nQuery: SELECT * FROM non_existent\nParameters: []",
    );
}

#[test]
fn test_invalid_number_of_parameters() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    let mut w = Cursor::new(Vec::<u8>::new());
    assert_eq!(
        format!(
            "{}",
            db.handle(
                &mut w,
                "SELECT ?, ?, @a",
                &[CLIValue::I64(1)],
                QueryMode::ReadOnly,
                QueryOptions::default(),
            )
            .unwrap_err()
        ),
        "Invalid number of parameters.
Query: SELECT ?, ?, @a
Parameters: [1]
Placeholders: [?, ?, @a]",
    );
}

fn select_regex<T, P>(db: &rusqlite::Connection, text: T, pattern: P, whole_word: bool, case_sensitive: bool) -> i64
where
    T: Into<CLIValue>,
    P: Into<CLIValue>,
{
    *select_all(
        db,
        &format!(
            r#"SELECT find_widget_compare_r{}{}(?, ?);"#,
            if whole_word { "_w" } else { "" },
            if case_sensitive { "_c" } else { "" }
        ),
        &[text.into(), pattern.into()],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
    .first()
    .unwrap()
}

#[test]
fn test_find_widget_compare_r() {
    let db = SQLite3Connection::connect(":memory:", false).unwrap();

    // Default
    assert_eq!(select_regex(db.con(), "abcd", "BC", false, false), 1);
    assert_eq!(select_regex(db.con(), "abcd", "BB", false, false), 0);

    // Whole word
    assert_eq!(select_regex(db.con(), "abc d", "ABC", true, false), 1);
    assert_eq!(select_regex(db.con(), "a bc d", "BC", true, false), 1);
    assert_eq!(select_regex(db.con(), "a bcd", "BCD", true, false), 1);
    assert_eq!(select_regex(db.con(), "abcd", "ABCD", true, false), 1);
    assert_eq!(select_regex(db.con(), "abcd", "ABC", true, false), 0);
    assert_eq!(select_regex(db.con(), "abcd", "", true, false), 0);

    // Case-sensitive
    assert_eq!(select_regex(db.con(), "abcd", "bc", false, true), 1);
    assert_eq!(select_regex(db.con(), "abcd", "BC", false, true), 0);

    // Whole word & case-sensitive
    assert_eq!(select_regex(db.con(), "a bc d", "bc", true, true), 1);
    assert_eq!(select_regex(db.con(), "abcd", "ABC", true, true), 0);
    assert_eq!(select_regex(db.con(), "abcd", "abc", true, true), 0);

    // Escape sequences, case-insensitive
    assert_eq!(select_regex(db.con(), "abcd", "\\w+", false, false), 1);
    assert_eq!(select_regex(db.con(), "abcd", "\\W+", false, false), 0);
    assert_eq!(select_regex(db.con(), "....", "\\w+", false, false), 0);
    assert_eq!(select_regex(db.con(), "....", "\\W+", false, false), 1);

    // Escape sequences, case-sensitive
    assert_eq!(select_regex(db.con(), "abcd", "\\w+", false, true), 1);
    assert_eq!(select_regex(db.con(), "abcd", "\\W+", false, true), 0);
    assert_eq!(select_regex(db.con(), "....", "\\w+", false, true), 0);
    assert_eq!(select_regex(db.con(), "....", "\\W+", false, true), 1);

    // Regex, number
    assert_eq!(select_regex(db.con(), 123, "123", false, true), 1);
    assert_eq!(select_regex(db.con(), 123, "\\d+", false, true), 1);
    assert_eq!(select_regex(db.con(), 123, "\\d+1", false, true), 0);

    // NULL
    assert_eq!(select_regex(db.con(), None::<i64>, "NULL", true, true), 1);
    assert_eq!(select_regex(db.con(), None::<i64>, "null", true, false), 1);

    // REAL
    assert_eq!(select_regex(db.con(), 1.23, "1.23", true, true), 1);
    assert_eq!(select_regex(db.con(), 1.23, "1.23", true, false), 1);
    assert_eq!(select_regex(db.con(), 1.23, "1.24", true, false), 0);

    // Invalid parameter types
    assert_eq!(select_regex(db.con(), "abcd", 0, false, false), 0);
}

fn handle(db: &mut SQLite3Connection, query: &str, params: &[CLIValue], mode: QueryMode) -> String {
    let mut w = vec![];
    db.handle(&mut w, query, params, mode, QueryOptions::default()).unwrap();
    let json = decode_msgpack_into_json(&w);
    regex::Regex::new(r#""time":[\d.]+"#)
        .unwrap()
        .replace(&json, r#""time":0"#)
        .to_string()
}

#[test]
fn test_handle_select() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    assert_eq!(
        handle(&mut db, "SELECT 1, 2", &[], QueryMode::ReadOnly),
        r#"{"records":{"1":[1],"2":[2]},"warnings":[],"time":0}"#
    );
}

#[test]
fn test_handle_table_schema() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    handle(&mut db, "CREATE TABLE t(x)", &[], QueryMode::ReadWrite);
    handle(
        &mut db,
        "EDITOR_PRAGMA table_schema",
        &["main".into(), "t".into()],
        QueryMode::ReadOnly,
    );
    handle(
        &mut db,
        "EDITOR_PRAGMA query_schema",
        &["SELECT * FROM t".into()],
        QueryMode::ReadOnly,
    );
}

#[test]
fn test_handle_table_schema_invalid_params() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    assert!(db
        .handle(
            &mut vec![],
            "EDITOR_PRAGMA table_schema",
            &[1.into(), 2.into()],
            QueryMode::ReadOnly,
            QueryOptions::default(),
        )
        .unwrap_err()
        .to_string()
        .contains("invalid argument"),);
    assert!(db
        .handle(
            &mut vec![],
            "EDITOR_PRAGMA query_schema",
            &[1.into()],
            QueryMode::ReadOnly,
            QueryOptions::default(),
        )
        .unwrap_err()
        .to_string()
        .contains("invalid argument"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_handle_load_extensions() {
    // sudo apt install -y libsqlite3-mod-spatialite
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    handle(
        &mut db,
        "EDITOR_PRAGMA load_extensions",
        &["mod_spatialite".into()],
        QueryMode::ReadOnly,
    );
}

#[test]
fn test_invalid_utf8_text() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
    let mut warnings = vec![];
    db._execute(
        "SELECT CAST(x'ff' AS TEXT)",
        &[],
        ExecMode::ReadWrite,
        QueryOptions::default(),
        &mut warnings,
    )
    .unwrap();
    assert_eq!(
        warnings,
        vec![InvalidUTF8 {
            text_lossy: "�".to_owned(),
            bytes: "ff".to_owned(),
            context: Some("SELECT CAST(x'ff' AS TEXT)".to_owned()),
        }],
    );
}

#[test]
fn test_cache_clear() {
    let mut db = SQLite3Connection::connect(":memory:", false).unwrap();

    // initialize the data_version
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "SELECT ?",
        &[1.into()],
        QueryMode::ReadOnly,
        QueryOptions::default(),
    )
    .unwrap();

    let count = db.pager().cache_clear_count;

    // readonly
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "SELECT ?",
        &[1.into()],
        QueryMode::ReadOnly,
        QueryOptions::default(),
    )
    .unwrap();
    assert_eq!(db.pager().cache_clear_count, count);

    // readwrite
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "CREATE TABLE t(x)",
        &[],
        QueryMode::ReadWrite,
        QueryOptions::default(),
    )
    .unwrap();
    assert_eq!(db.pager().cache_clear_count, count + 1);

    // script
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "CREATE TABLE u(x)",
        &[],
        QueryMode::Script,
        QueryOptions::default(),
    )
    .unwrap();
    assert_eq!(db.pager().cache_clear_count, count + 2);
}
