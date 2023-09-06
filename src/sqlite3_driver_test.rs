use std::{collections::HashSet, io::Cursor, vec};

use crate::{
    literal::{Blob, Literal},
    request_type::QueryMode,
    sqlite3_driver::{from_utf8_lossy, read_msgpack_into_json, ForeignKey, InvalidUTF8, SQLite3Driver, Table},
};

impl<'a> From<rusqlite::types::ValueRef<'a>> for Literal {
    fn from(value: rusqlite::types::ValueRef<'a>) -> Self {
        match value {
            rusqlite::types::ValueRef::Blob(v) => Literal::Blob(Blob(v.to_vec())),
            rusqlite::types::ValueRef::Integer(v) => Literal::I64(v),
            rusqlite::types::ValueRef::Null => Literal::Nil,
            rusqlite::types::ValueRef::Real(v) => Literal::F64(v),
            rusqlite::types::ValueRef::Text(v) => Literal::String(String::from_utf8_lossy(v).to_string()),
        }
    }
}

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
fn test_select_params() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    let params: Vec<Literal> = vec![Literal::Nil, "a".into(), 123.into(), 1.23.into(), vec![1, 2, 3].into()];
    assert_eq!(
        db.select_one("SELECT ?, ?, ?, ?, ?", &params, |v| {
            Ok((
                Literal::from(v.get_ref_unwrap(0)),
                Literal::from(v.get_ref_unwrap(1)),
                Literal::from(v.get_ref_unwrap(2)),
                Literal::from(v.get_ref_unwrap(3)),
                Literal::from(v.get_ref_unwrap(4)),
            ))
        },)
            .unwrap(),
        (
            params[0].to_owned(),
            params[1].to_owned(),
            params[2].to_owned(),
            params[3].to_owned(),
            params[4].to_owned()
        )
    );
}

#[test]
fn test_execute_params() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    let params: Vec<Literal> = vec![Literal::Nil, "a".into(), 123.into(), 1.23.into(), vec![1, 2, 3].into()];
    let mut warnings = vec![];
    assert_eq!(
        read_msgpack_into_json(std::io::Cursor::new(
            db.execute("SELECT ? as a, ? as b, ? as c, ? as d, ? as e", &params, &mut warnings)
                .unwrap()
        )),
        r#"{"a":[null],"b":["a"],"c":[123],"d":[1.23],"e":[[1,2,3]]}"#
    );
    assert_eq!(warnings, vec![]);
}

#[test]
fn test_values() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_eq!(
        convert_msgpack_to_json(
            &db.execute(
                r#"
WITH
    temp_table (column1, column2) AS (VALUES (1, 2), (3, 4))
SELECT * FROM temp_table;
"#,
                &[],
                &mut vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        "{\"column1\":[1,3],\"column2\":[2,4]}"
    );
}

#[cfg(test)]
mod test_table_schema {
    use crate::sqlite3_driver::{
        DfltValue, IndexColumn, SQLite3Driver, TableSchema, TableSchemaColumn, TableSchemaColumnForeignKey,
        TableSchemaIndex,
    };

    #[test]
    fn test_simple() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

        db.execute(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
            &[],
            &mut vec![],
        )
        .unwrap();

        assert_eq!(
            db.table_schema("t").unwrap().0,
            TableSchema {
                name: "t".to_owned(),
                type_: "table".to_owned(),
                schema: "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT".to_owned(),
                has_rowid_column: true,
                strict: true,
                columns: vec![TableSchemaColumn {
                    cid: 0,
                    dflt_value: None,
                    name: "x".to_owned(),
                    notnull: true,
                    type_: "INTEGER".to_owned(),
                    pk: true,
                    auto_increment: false,
                    foreign_keys: vec![],
                    hidden: 0,
                }],
                indexes: vec![],
                triggers: vec![],
            },
        );
    }

    #[test]
    fn test_foreign_key() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
            &[],
            &mut vec![],
        )
        .unwrap();
        db.execute("CREATE TABLE u(y INTEGER NOT NULL REFERENCES t(x))", &[], &mut vec![])
            .unwrap();
        assert_eq!(
            db.table_schema("u").unwrap().0.columns[0].foreign_keys[0],
            TableSchemaColumnForeignKey {
                id: 0,
                seq: 0,
                table: "t".to_owned(),
                from: "y".to_owned(),
                to: Some("x".to_owned()),
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned()
            }
        );
    }

    #[test]
    fn test_foreign_key_to_none() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
            &[],
            &mut vec![],
        )
        .unwrap();
        db.execute("CREATE TABLE v(x INTEGER NOT NULL REFERENCES t)", &[], &mut vec![])
            .unwrap();
        assert_eq!(
            db.table_schema("v").unwrap().0.columns[0].foreign_keys[0],
            TableSchemaColumnForeignKey {
                id: 0,
                seq: 0,
                table: "t".to_owned(),
                from: "x".to_owned(),
                to: None, // test this
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned()
            }
        );
    }

    #[test]
    fn test_auto_increment() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT)",
            &[],
            &mut vec![],
        )
        .unwrap();
        db.execute("INSERT INTO t DEFAULT VALUES", &[], &mut vec![]).unwrap();
        assert_eq!(db.table_schema("t").unwrap().0.columns[0].auto_increment, true);
    }

    #[test]
    fn test_not_auto_increment() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute("CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY)", &[], &mut vec![])
            .unwrap();
        db.execute("INSERT INTO t DEFAULT VALUES", &[], &mut vec![]).unwrap();
        assert_eq!(db.table_schema("t").unwrap().0.columns[0].auto_increment, false);
    }

    #[test]
    fn test_default_value() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute("CREATE TABLE t(a DEFAULT NULL, b DEFAULT 1, c DEFAULT 1.2, d DEFAULT 'a', e DEFAULT x'1234', f DEFAULT (1 + 2))", &[], &mut vec![]).unwrap();
        assert_eq!(
            db.table_schema("t")
                .unwrap()
                .0
                .columns
                .into_iter()
                .map(|v| v.dflt_value)
                .collect::<Vec<_>>(),
            vec![
                Some(DfltValue::String("NULL".to_owned())),
                Some(DfltValue::String("1".to_owned())),
                Some(DfltValue::String("1.2".to_owned())),
                Some(DfltValue::String("'a'".to_owned())),
                Some(DfltValue::String("x'1234'".to_owned())),
                Some(DfltValue::String("1 + 2".to_owned())),
            ]
        );
    }

    #[test]
    fn test_index() {
        let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute("CREATE TABLE t(x UNIQUE)", &[], &mut vec![]).unwrap();
        db.execute("CREATE INDEX idx1 ON t(x)", &[], &mut vec![]).unwrap();
        assert_eq!(
            db.table_schema("t").unwrap().0.indexes,
            vec![
                TableSchemaIndex {
                    seq: 0,
                    name: "idx1".to_owned(),
                    unique: 0,
                    origin: "c".to_owned(),
                    partial: 0,
                    columns: Some(vec![IndexColumn {
                        seqno: 0,
                        cid: 0,
                        name: Some("x".to_owned()),
                    }]),
                    schema: Some("CREATE INDEX idx1 ON t(x)".to_owned()),
                },
                TableSchemaIndex {
                    seq: 1,
                    name: "sqlite_autoindex_t_1".to_owned(),
                    unique: 1,
                    origin: "u".to_owned(),
                    partial: 0,
                    columns: Some(vec![IndexColumn {
                        seqno: 0,
                        cid: 0,
                        name: Some("x".to_owned()),
                    }]),
                    schema: None,
                },
            ]
        );
    }
}

#[test]
fn test_list_foreign_keys() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.execute(
        "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
        &[],
        &mut vec![],
    )
    .unwrap();
    db.execute("CREATE TABLE v(x INTEGER NOT NULL REFERENCES t)", &[], &mut vec![])
        .unwrap();
    assert_eq!(
        db.list_foreign_keys().unwrap().0,
        vec![ForeignKey {
            name: "v".to_owned(),
            table: "t".to_owned(),
            from: "x".to_owned(),
            to: None,
        }],
    );
}

#[test]
fn test_list_tables() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.execute(
        "CREATE TABLE t1(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
        &mut vec![],
    )
    .unwrap();
    db.execute(
        "CREATE TABLE t2(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
        &mut vec![],
    )
    .unwrap();
    assert_eq!(
        db.list_tables().unwrap().0.into_iter().collect::<HashSet<Table>>(),
        HashSet::from([
            Table {
                name: "t1".to_owned(),
                type_: "table".to_owned()
            },
            Table {
                name: "t2".to_owned(),
                type_: "table".to_owned()
            },
        ]),
    );
}

#[test]
fn test_json() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_eq!(
        db.select_one(
            r#"select json_extract('{"foo": {"bar": 123}}', '$.foo.bar');"#,
            &[],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        123
    );
}

#[test]
fn test_database_label() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_ne!(db.database_label(), "");
}

#[test]
fn test_query_error() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    let mut w = Cursor::new(Vec::<u8>::new());
    assert_eq!(
        format!(
            "{}",
            db.handle(
                &mut w,
                "SELECT * FROM non_existent",
                &[],
                crate::request_type::QueryMode::ReadOnly
            )
            .unwrap_err()
        ),
        "no such table: non_existent\nQuery: SELECT * FROM non_existent\nParams: []",
    );
}

#[test]
fn test_transaction_success() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        r#"
BEGIN;
CREATE TABLE t(x);
INSERT INTO t VALUES (1);
COMMIT;"#,
        &[],
        crate::request_type::QueryMode::Script,
    )
    .unwrap();

    assert_eq!(
        db.select_one("SELECT * FROM t", &[], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn test_transaction_rollback() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    match db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        r#"
-- prepare
CREATE TABLE t(x);
CREATE VIEW IF NOT EXISTS invalid_view AS SELECT * FROM non_existent;

-- Begin a transaction and raise an error.
BEGIN;
INSERT INTO t VALUES (1);
SELECT * FROM invalid_view;
COMMIT;"#,
        &[],
        crate::request_type::QueryMode::Script,
    ) {
        Ok(_) => panic!(),
        Err(err) => {
            if !err.to_string().contains("no such table") {
                panic!("{}", err);
            }
        }
    }

    match db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        r#"
-- The previous transaction should have been aborted.
BEGIN;
INSERT INTO t VALUES (1);
SELECT * FROM invalid_view;
COMMIT;"#,
        &[],
        crate::request_type::QueryMode::Script,
    ) {
        Ok(_) => panic!(),
        Err(err) => {
            if !err.to_string().contains("no such table") {
                panic!("{}", err);
            }
        }
    }

    assert_eq!(
        db.select_one("SELECT count(*) FROM t", &[], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn test_uncommitted_transaction() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        r#"
CREATE TABLE t(x);
BEGIN;
INSERT INTO t VALUES (1);
"#,
        &[],
        crate::request_type::QueryMode::Script,
    )
    .unwrap();

    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "BEGIN;",
        &[],
        crate::request_type::QueryMode::Script,
    )
    .unwrap();

    assert_eq!(
        db.select_one("SELECT count(*) FROM t", &[], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn test_from_utf8_lossy() {
    let mut warnings = vec![];
    assert_eq!(from_utf8_lossy(&[b'a', 255], |err| warnings.push(err)), "a\u{FFFD}");
    assert_eq!(
        warnings,
        vec![InvalidUTF8 {
            bytes: "61ff".to_owned(),
            text_lossy: "a\u{FFFD}".to_owned(),
            context: None,
        }]
    );
}

fn select_regex<T, P, W, C>(db: &SQLite3Driver, text: T, pattern: P, whole_word: W, case_sensitive: C) -> i64
where
    T: Into<Literal>,
    P: Into<Literal>,
    W: Into<Literal>,
    C: Into<Literal>,
{
    db.select_one(
        r#"SELECT find_widget_regexp(?, ?, ?, ?);"#,
        &[text.into(), pattern.into(), whole_word.into(), case_sensitive.into()],
        |row| row.get::<_, i64>(0),
    )
    .unwrap()
}

#[test]
fn test_find_widget_regexp() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    // Default
    assert_eq!(select_regex(&db, "abcd", "BC", 0, 0), 1);
    assert_eq!(select_regex(&db, "abcd", "BB", 0, 0), 0);

    // Whole word
    assert_eq!(select_regex(&db, "abc d", "ABC", 1, 0), 1);
    assert_eq!(select_regex(&db, "a bc d", "BC", 1, 0), 1);
    assert_eq!(select_regex(&db, "a bcd", "BCD", 1, 0), 1);
    assert_eq!(select_regex(&db, "abcd", "ABCD", 1, 0), 1);
    assert_eq!(select_regex(&db, "abcd", "ABC", 1, 0), 0);
    assert_eq!(select_regex(&db, "abcd", "", 1, 0), 0);

    // Case-sensitive
    assert_eq!(select_regex(&db, "abcd", "bc", 0, 1), 1);
    assert_eq!(select_regex(&db, "abcd", "BC", 0, 1), 0);

    // Whole word & case-sensitive
    assert_eq!(select_regex(&db, "a bc d", "bc", 1, 1), 1);
    assert_eq!(select_regex(&db, "abcd", "ABC", 1, 1), 0);
    assert_eq!(select_regex(&db, "abcd", "abc", 1, 1), 0);

    // Escape sequences, case-insensitive
    assert_eq!(select_regex(&db, "abcd", "\\w+", 0, 0), 1);
    assert_eq!(select_regex(&db, "abcd", "\\W+", 0, 0), 0);
    assert_eq!(select_regex(&db, "....", "\\w+", 0, 0), 0);
    assert_eq!(select_regex(&db, "....", "\\W+", 0, 0), 1);

    // Escape sequences, case-sensitive
    assert_eq!(select_regex(&db, "abcd", "\\w+", 0, 1), 1);
    assert_eq!(select_regex(&db, "abcd", "\\W+", 0, 1), 0);
    assert_eq!(select_regex(&db, "....", "\\w+", 0, 1), 0);
    assert_eq!(select_regex(&db, "....", "\\W+", 0, 1), 1);

    // Regex, number
    assert_eq!(select_regex(&db, 123, "123", 0, 1), 1);
    assert_eq!(select_regex(&db, 123, "\\d+", 0, 1), 1);
    assert_eq!(select_regex(&db, 123, "\\d+1", 0, 1), 0);

    // NULL
    assert_eq!(select_regex(&db, None::<i64>, "NULL", 1, 1), 1);
    assert_eq!(select_regex(&db, None::<i64>, "null", 1, 0), 1);

    // REAL
    assert_eq!(select_regex(&db, 1.23, "1.23", 1, 1), 1);
    assert_eq!(select_regex(&db, 1.23, "1.23", 1, 0), 1);
    assert_eq!(select_regex(&db, 1.23, "1.24", 1, 0), 0);

    // Invalid parameter types
    assert_eq!(select_regex(&db, "abcd", "abcd", "a", 0), 0);
    assert_eq!(select_regex(&db, "abcd", "abcd", 0, "a"), 0);
    assert_eq!(select_regex(&db, "abcd", 0, 0, 0), 0);
}

fn handle(db: &SQLite3Driver, query: &str, params: &[Literal], mode: QueryMode) -> String {
    let mut w = vec![];
    db.handle(&mut w, query, params, mode).unwrap();
    regex::Regex::new(r#""time":[\d.]+"#)
        .unwrap()
        .replace(&read_msgpack_into_json(std::io::Cursor::new(w)), r#""time":0"#)
        .to_string()
}

#[test]
fn test_handle_select() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_eq!(
        handle(&db, "SELECT 1, 2", &[], QueryMode::ReadOnly),
        r#"{"records":{"1":[1],"2":[2]},"warnings":[],"time":0}"#
    );
}

#[test]
fn test_handle_table_schema() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    handle(&db, "CREATE TABLE t(x)", &[], QueryMode::ReadWrite);
    handle(&db, "EDITOR_PRAGMA table_schema", &["t".into()], QueryMode::ReadOnly);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_handle_load_extensions() {
    // sudo apt install -y libsqlite3-mod-spatialite
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    handle(
        &db,
        "EDITOR_PRAGMA load_extensions",
        &["mod_spatialite".into()],
        QueryMode::ReadOnly,
    );
}

#[test]
fn test_invalid_utf8_text() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    let mut warnings = vec![];
    db.execute("SELECT CAST(x'ff' AS TEXT)", &[], &mut warnings).unwrap();
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
fn test_invalid_utf8_table_name() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    let mut warnings = vec![];
    let mut query = "CREATE TABLE ab(x)".as_bytes().to_vec();
    query[14] = 255;
    let query = unsafe { String::from_utf8_unchecked(query) };
    db.execute(&query, &[], &mut warnings).unwrap();
    assert_eq!(
        db.list_tables().unwrap(),
        (
            vec![Table {
                name: "a�".to_owned(),
                type_: "table".to_owned(),
            }],
            vec![InvalidUTF8 {
                text_lossy: "a�".to_owned(),
                bytes: "61ff".to_owned(),
                context: Some("pragma_table_list.name".to_owned()),
            }],
        )
    );
}
