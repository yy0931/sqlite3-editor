use std::{collections::HashSet, io::Cursor, vec};

use crate::{
    literal::{Blob, Literal},
    sqlite3_driver::{SQLite3Driver, Table, TableSchema, TableSchemaColumn},
};

impl<'a> From<rusqlite::types::ValueRef<'a>> for Literal {
    fn from(value: rusqlite::types::ValueRef<'a>) -> Self {
        match value {
            rusqlite::types::ValueRef::Blob(v) => Literal::Blob(Blob(v.to_vec())),
            rusqlite::types::ValueRef::Integer(v) => Literal::I64(v),
            rusqlite::types::ValueRef::Null => Literal::Nil,
            rusqlite::types::ValueRef::Real(v) => Literal::F64(v),
            // FIXME: utf-16?
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

    let params = vec![
        Literal::Nil,
        Literal::String("a".to_owned()),
        Literal::I64(123),
        Literal::F64(1.23),
        Literal::Blob(Blob(vec![1, 2, 3])),
    ];
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

#[test]
fn test_table_schema() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    // a random table
    db.execute("CREATE TABLE t(x INTEGER NOT NULL) STRICT", &[], &mut vec![])
        .unwrap();

    assert_eq!(
        db.table_schema("t").unwrap().0,
        TableSchema {
            name: "t".to_owned(),
            type_: "table".to_owned(),
            schema: "CREATE TABLE t(x INTEGER NOT NULL) STRICT".to_owned(),
            has_rowid_column: true,
            strict: true,
            columns: vec![TableSchemaColumn {
                cid: 0,
                dflt_value: None,
                name: "x".to_owned(),
                notnull: true,
                type_: "INTEGER".to_owned(),
                pk: false,
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
    assert!(w.get_mut().is_empty());
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
