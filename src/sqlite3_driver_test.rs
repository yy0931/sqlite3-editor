use std::{collections::HashSet, io::Cursor, vec};

use crate::{
    literal::{Blob, Literal},
    sqlite3_driver::{SQLite3Driver, Table, TableSchema, TableSchemaColumn},
};

fn convert_msgpack_to_json(input: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = vec![];
    serde_transcode::transcode(
        &mut rmp_serde::Deserializer::from_read_ref(input),
        &mut serde_json::Serializer::new(&mut buf),
    )
    .unwrap();
    Ok(String::from_utf8(buf)?.to_string())
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
    db.execute("CREATE TABLE t(x INTEGER NOT NULL) STRICT", &[]).unwrap();

    assert_eq!(
        db.table_schema("t"),
        Ok(TableSchema {
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
        }),
    );
}

#[test]
fn test_list_tables() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.execute(
        "CREATE TABLE t1(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
    )
    .unwrap();
    db.execute(
        "CREATE TABLE t2(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
    )
    .unwrap();
    assert_eq!(
        db.list_tables().unwrap().into_iter().collect::<HashSet<Table>>(),
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
                "SELECT * FROM nonExistent",
                &[],
                crate::request_type::QueryMode::ReadOnly
            )
            .unwrap_err()
        ),
        "no such table: nonExistent\nQuery: SELECT * FROM nonExistent\nParams: []",
    );
}
