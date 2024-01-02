use std::{collections::HashSet, io::Cursor, rc::Rc, vec};

use crate::{
    literal::Literal,
    request_type::QueryMode,
    sqlite3_driver::{
        from_utf8_lossy, read_msgpack_into_json, EntityRelationship, ExecMode, InvalidUTF8, SQLite3Driver, Table,
        TableType,
    },
};

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
        db.select_all("SELECT ?, ?, ?, ?, ?", &params, |v| {
            Ok((
                Literal::from(v.get_ref_unwrap(0)),
                Literal::from(v.get_ref_unwrap(1)),
                Literal::from(v.get_ref_unwrap(2)),
                Literal::from(v.get_ref_unwrap(3)),
                Literal::from(v.get_ref_unwrap(4)),
            ))
        })
        .unwrap()
        .first()
        .unwrap(),
        &(
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
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    let params: Vec<Literal> = vec![Literal::Nil, "a".into(), 123.into(), 1.23.into(), vec![1, 2, 3].into()];
    let mut warnings = vec![];
    assert_eq!(
        read_msgpack_into_json(std::io::Cursor::new(
            db.execute(
                "SELECT ? as a, ? as b, ? as c, ? as d, ? as e",
                &params,
                ExecMode::ReadWrite,
                &mut warnings
            )
            .unwrap()
        )),
        r#"{"a":[null],"b":["a"],"c":[123],"d":[1.23],"e":[[1,2,3]]}"#
    );
    assert_eq!(warnings, vec![]);
}

#[test]
fn test_values() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_eq!(
        convert_msgpack_to_json(
            &db.execute(
                r#"
WITH
    temp_table (column1, column2) AS (VALUES (1, 2), (3, 4))
SELECT * FROM temp_table;
"#,
                &[],
                ExecMode::ReadWrite,
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
    use std::{collections::HashMap, rc::Rc};

    use crate::sqlite3_driver::{
        ColumnOriginAndIsRowId, DfltValue, ExecMode, IndexColumn, SQLite3Driver, TableSchema, TableSchemaColumn,
        TableSchemaColumnForeignKey, TableSchemaIndex, TableSchemaTrigger, TableType,
    };

    #[test]
    fn test_table() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

        db.execute(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();

        assert_eq!(
            db.table_schema("main", "t").unwrap().0.unwrap(),
            TableSchema {
                name: Some("t".to_owned()),
                type_: TableType::Table,
                schema: Some("CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT".to_owned()),
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
                    foreign_keys: Rc::new(vec![]),
                    hidden: 0,
                }],
                indexes: vec![],
                triggers: vec![],
                column_origins: None,
                custom_query: None,
            },
        );
    }

    #[test]
    fn test_view() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

        db.execute("CREATE TABLE t(x)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        db.execute(
            "CREATE VIEW u AS SELECT x as y FROM t",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();

        assert_eq!(
            db.table_schema("main", "u").unwrap().0.unwrap(),
            TableSchema {
                name: Some("u".to_owned()),
                type_: TableType::View,
                schema: Some("CREATE VIEW u AS SELECT x as y FROM t".to_owned()),
                has_rowid_column: false,
                strict: false,
                columns: vec![TableSchemaColumn {
                    cid: 0,
                    dflt_value: None,
                    name: "y".to_owned(),
                    notnull: false,
                    type_: "".to_owned(),
                    pk: false,
                    auto_increment: false,
                    foreign_keys: Rc::new(vec![]),
                    hidden: 0,
                }],
                indexes: vec![],
                triggers: vec![],
                column_origins: Some(HashMap::from([(
                    "y".to_owned(),
                    ColumnOriginAndIsRowId {
                        database: "main".to_owned(),
                        table: "t".to_owned(),
                        column: "x".to_owned(),
                        is_rowid: false,
                    }
                )])),
                custom_query: None,
            },
        );
    }

    #[test]
    fn test_foreign_key() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();
        db.execute(
            "CREATE TABLE u(y INTEGER NOT NULL REFERENCES t(x))",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();
        assert_eq!(
            db.table_schema("main", "u").unwrap().0.unwrap().columns[0].foreign_keys[0],
            TableSchemaColumnForeignKey {
                id: 0,
                seq: 0,
                table: "t".to_owned(),
                to: "x".to_owned(),
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned(),
            }
        );
    }

    #[test]
    fn test_foreign_key_to_none() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();
        db.execute(
            "CREATE TABLE v(x INTEGER NOT NULL REFERENCES t)",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();
        assert_eq!(
            db.table_schema("main", "v").unwrap().0.unwrap().columns[0].foreign_keys[0],
            TableSchemaColumnForeignKey {
                id: 0,
                seq: 0,
                table: "t".to_owned(),
                to: "x".to_owned(),
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned(),
            }
        );
    }

    #[test]
    fn test_auto_increment() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT)",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();
        db.execute("INSERT INTO t DEFAULT VALUES", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        assert!(db.table_schema("main", "t").unwrap().0.unwrap().columns[0].auto_increment);
    }

    #[test]
    fn test_not_auto_increment() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute(
            "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY)",
            &[],
            ExecMode::ReadWrite,
            &mut vec![],
        )
        .unwrap();
        db.execute("INSERT INTO t DEFAULT VALUES", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        assert!(!db.table_schema("main", "t").unwrap().0.unwrap().columns[0].auto_increment);
    }

    #[test]
    fn test_default_value() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute("CREATE TABLE t(a DEFAULT NULL, b DEFAULT 1, c DEFAULT 1.2, d DEFAULT 'a', e DEFAULT x'1234', f DEFAULT (1 + 2))", &[], ExecMode::ReadWrite, &mut vec![]).unwrap();
        assert_eq!(
            db.table_schema("main", "t")
                .unwrap()
                .0
                .unwrap()
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
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute("CREATE TABLE t(x UNIQUE)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        db.execute("CREATE INDEX idx1 ON t(x)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        assert_eq!(
            db.table_schema("main", "t").unwrap().0.unwrap().indexes,
            vec![
                TableSchemaIndex {
                    seq: 0,
                    name: "idx1".to_owned(),
                    unique: 0,
                    origin: "c".to_owned(),
                    partial: 0,
                    columns: vec![IndexColumn {
                        seqno: 0,
                        cid: 0,
                        name: Some("x".to_owned()),
                    }],
                    schema: Some("CREATE INDEX idx1 ON t(x)".to_owned()),
                },
                TableSchemaIndex {
                    seq: 1,
                    name: "sqlite_autoindex_t_1".to_owned(),
                    unique: 1,
                    origin: "u".to_owned(),
                    partial: 0,
                    columns: vec![IndexColumn {
                        seqno: 0,
                        cid: 0,
                        name: Some("x".to_owned()),
                    }],
                    schema: None,
                },
            ]
        );
    }

    #[test]
    fn test_trigger() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
        db.execute("CREATE TABLE t(x)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        let sql = "CREATE TRIGGER trigger_insert AFTER INSERT ON t BEGIN SELECT 1; END";
        db.execute(sql, &[], ExecMode::ReadWrite, &mut vec![]).unwrap();
        assert_eq!(
            db.table_schema("main", "t").unwrap().0.unwrap().triggers,
            vec![TableSchemaTrigger {
                name: "trigger_insert".to_owned(),
                sql: sql.to_owned(),
            },]
        );
    }

    #[test]
    fn test_query_schema() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

        db.execute("CREATE TABLE t(x INTEGER)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        assert_eq!(
            db.query_schema("SELECT 1, x FROM t").unwrap().0,
            TableSchema {
                name: None,
                type_: TableType::CustomQuery,
                schema: None,
                has_rowid_column: false,
                strict: false,
                columns: vec![
                    TableSchemaColumn {
                        cid: 0,
                        dflt_value: None,
                        name: "1".to_owned(),
                        notnull: false,
                        type_: "".to_owned(),
                        pk: false,
                        auto_increment: false,
                        foreign_keys: Rc::new(vec![]),
                        hidden: 0,
                    },
                    TableSchemaColumn {
                        cid: 1,
                        dflt_value: None,
                        name: "x".to_owned(),
                        notnull: false,
                        type_: "".to_owned(),
                        pk: false,
                        auto_increment: false,
                        foreign_keys: Rc::new(vec![]),
                        hidden: 0,
                    }
                ],
                indexes: vec![],
                triggers: vec![],
                column_origins: Some(HashMap::from([(
                    "x".to_owned(),
                    ColumnOriginAndIsRowId {
                        database: "main".to_owned(),
                        table: "t".to_owned(),
                        column: "x".to_owned(),
                        is_rowid: false,
                    },
                )])),
                custom_query: Some("SELECT 1, x FROM t".to_owned()),
            },
        );
    }

    #[test]
    fn test_temp_table() {
        let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

        db.execute("CREATE TABLE t(x)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        db.execute("CREATE TEMP TABLE t(y)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
        db.execute("CREATE TEMP TABLE u(y)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();

        assert_eq!(
            db.table_schema("main", "t").unwrap().0.unwrap().schema.unwrap(),
            "CREATE TABLE t(x)"
        );
        assert_eq!(
            db.table_schema("temp", "t").unwrap().0.unwrap().schema.unwrap(),
            "CREATE TABLE t(y)"
        );
        assert_eq!(
            db.table_schema("temp", "u").unwrap().0.unwrap().schema.unwrap(),
            "CREATE TABLE u(y)"
        );
    }

    mod test_indirect_foreign_key {
        use std::{collections::HashMap, rc::Rc};

        use crate::sqlite3_driver::{ColumnOriginAndIsRowId, ExecMode, SQLite3Driver, TableSchemaColumnForeignKey};

        #[test]
        fn test_table_schema() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

            db.execute(
                "CREATE TABLE t1(x INTEGER PRIMARY KEY)",
                &[],
                ExecMode::ReadWrite,
                &mut vec![],
            )
            .unwrap();
            db.execute(
                "CREATE TABLE t2(y INTEGER REFERENCES t1(x))",
                &[],
                ExecMode::ReadWrite,
                &mut vec![],
            )
            .unwrap();
            db.execute(
                "CREATE VIEW table_name AS SELECT y as z FROM t2;",
                &[],
                ExecMode::ReadWrite,
                &mut vec![],
            )
            .unwrap();
            let schema = db.table_schema("main", "table_name").unwrap().0.unwrap();
            assert_eq!(
                schema.column_origins,
                Some(HashMap::from([(
                    "z".to_owned(),
                    ColumnOriginAndIsRowId {
                        database: "main".to_owned(),
                        table: "t2".to_owned(),
                        column: "y".to_owned(),
                        is_rowid: false,
                    }
                )]))
            );
            assert_eq!(schema.columns[0].name, "z");
            assert_eq!(
                schema.columns[0].foreign_keys,
                Rc::new(vec![TableSchemaColumnForeignKey {
                    id: 0,
                    seq: 0,
                    table: "t1".to_owned(),
                    to: "x".to_owned(),
                    on_update: "NO ACTION".to_owned(),
                    on_delete: "NO ACTION".to_owned(),
                    match_: "NONE".to_owned(),
                }])
            );
        }

        #[test]
        fn test_query_schema() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

            db.execute(
                "CREATE TABLE t1(x INTEGER PRIMARY KEY)",
                &[],
                ExecMode::ReadWrite,
                &mut vec![],
            )
            .unwrap();
            db.execute(
                "CREATE TABLE t2(y INTEGER REFERENCES t1(x))",
                &[],
                ExecMode::ReadWrite,
                &mut vec![],
            )
            .unwrap();
            db.execute(
                "CREATE VIEW table_name AS SELECT y as z FROM t2;",
                &[],
                ExecMode::ReadWrite,
                &mut vec![],
            )
            .unwrap();
            let schema = db.query_schema("SELECT * FROM table_name").unwrap().0;
            assert_eq!(
                schema.column_origins,
                Some(HashMap::from([(
                    "z".to_owned(),
                    ColumnOriginAndIsRowId {
                        database: "main".to_owned(),
                        table: "t2".to_owned(),
                        column: "y".to_owned(),
                        is_rowid: false,
                    }
                )]))
            );
            assert_eq!(schema.columns[0].name, "z");
            assert_eq!(
                schema.columns[0].foreign_keys,
                Rc::new(vec![TableSchemaColumnForeignKey {
                    id: 0,
                    seq: 0,
                    table: "t1".to_owned(),
                    to: "x".to_owned(),
                    on_update: "NO ACTION".to_owned(),
                    on_delete: "NO ACTION".to_owned(),
                    match_: "NONE".to_owned(),
                }])
            );
        }
    }

    mod test_rowid_alias {
        use crate::{
            column_origin::ColumnOrigin,
            sqlite3_driver::{escape_sql_identifier, get_string, ExecMode, SQLite3Driver},
        };

        fn execute(db: &mut SQLite3Driver, query: &str) {
            db.execute(query, &[], ExecMode::ReadWrite, &mut vec![]).unwrap();
        }

        fn check_is_rowid(db: &SQLite3Driver, table: &str, column: &str, is_rowid: bool) {
            assert_eq!(
                db.is_rowid(&ColumnOrigin::new("main", table, column), &mut vec![])
                    .unwrap(),
                is_rowid
            );
        }

        fn check_is_alias_to_rowid(db: &mut SQLite3Driver, table: &str, column: &str, yes: bool) {
            execute(db, &format!("INSERT INTO {table} DEFAULT VALUES"));
            assert_eq!(
                db.select_all(
                    &format!(
                        "SELECT typeof({}) FROM {}",
                        escape_sql_identifier(column),
                        escape_sql_identifier(table)
                    ),
                    &[],
                    |row| { get_string(row, 0, |_| {}) }
                )
                .unwrap()
                .first()
                .unwrap(),
                if yes { "integer" } else { "null" }
            );
        }

        #[test]
        fn test_single_integer_pk() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(x INTEGER PRIMARY KEY)");
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&mut db, "t", "x", true);
        }

        #[test]
        fn test_single_integer_not_null_pk() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY)");
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&mut db, "t", "x", true);
        }

        #[test]
        fn test_integer_unique_pk() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(x INTEGER UNIQUE PRIMARY KEY)");
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&mut db, "t", "x", true);
        }

        #[test]
        fn test_multiple_integer_pk() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(x INTEGER, y INTEGER, PRIMARY KEY (x, y))");
            check_is_rowid(&db, "t", "x", false);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&mut db, "t", "x", false);
        }

        #[test]
        fn test_int_pk() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(x INT PRIMARY KEY)");
            check_is_rowid(&db, "t", "x", false);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&mut db, "t", "x", false);
        }

        #[test]
        fn test_lowercase_integer_pk() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(x integer primary key)");
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&mut db, "t", "x", true);
        }

        #[test]
        fn test_shadowed_rowid() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(rowid TEXT)");
            check_is_rowid(&db, "t", "rowid", false);
            check_is_rowid(&db, "t", "_rowid_", true);
            check_is_alias_to_rowid(&mut db, "t", "rowid", false);
        }

        #[test]
        fn test_shadowed_rowid_and_oid() {
            let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
            execute(&mut db, "CREATE TABLE t(rowid TEXT, oid TEXT)");
            check_is_rowid(&db, "t", "rowid", false);
            check_is_rowid(&db, "t", "oid", false);
            check_is_rowid(&db, "t", "_rowid_", true);
            check_is_alias_to_rowid(&mut db, "t", "oid", false);
        }
    }
}

#[test]
fn test_list_entity_relationships() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.execute(
        "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT",
        &[],
        ExecMode::ReadWrite,
        &mut vec![],
    )
    .unwrap();
    db.execute(
        "CREATE TABLE v(x INTEGER NOT NULL REFERENCES t)",
        &[],
        ExecMode::ReadWrite,
        &mut vec![],
    )
    .unwrap();
    assert_eq!(
        db.list_entity_relationships().unwrap().0,
        vec![EntityRelationship {
            source: "v".to_owned(),
            target: "t".to_owned(),
            source_column: "x".to_owned(),
            target_column: "x".to_owned(),
        }],
    );
}

#[test]
fn test_list_tables() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    db.execute(
        "CREATE TABLE t1(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
        ExecMode::ReadWrite,
        &mut vec![],
    )
    .unwrap();
    db.execute(
        "CREATE TABLE t2(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
        ExecMode::ReadWrite,
        &mut vec![],
    )
    .unwrap();
    db.execute(
        "CREATE TEMP TABLE t3(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        &[],
        ExecMode::ReadWrite,
        &mut vec![],
    )
    .unwrap();
    assert_eq!(
        db.list_tables(false).unwrap().0.into_iter().collect::<HashSet<Table>>(),
        HashSet::from([
            Table {
                database: Rc::new("main".to_owned()),
                name: Rc::new("t1".to_owned()),
                type_: TableType::Table,
                column_names: vec!["x".to_owned()],
            },
            Table {
                database: Rc::new("main".to_owned()),
                name: Rc::new("t2".to_owned()),
                type_: TableType::Table,
                column_names: vec!["x".to_owned()],
            },
            Table {
                database: Rc::new("temp".to_owned()),
                name: Rc::new("t3".to_owned()),
                type_: TableType::Table,
                column_names: vec![],
            },
        ]),
    );
}

#[test]
fn test_json() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_eq!(
        db.select_all(
            r#"select json_extract('{"foo": {"bar": 123}}', '$.foo.bar');"#,
            &[],
            |row| row.get::<_, i64>(0)
        )
        .unwrap()
        .first()
        .unwrap(),
        &123
    );
}

#[test]
fn test_database_label() {
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_ne!(db.database_label, "");
}

#[test]
fn test_query_error() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
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
        "no such table: non_existent\nQuery: SELECT * FROM non_existent\nParameters: []",
    );
}

#[test]
fn test_transaction_success() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
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
        db.select_all("SELECT * FROM t", &[], |row| row.get::<_, i64>(0))
            .unwrap()
            .first()
            .unwrap(),
        &1
    );
}

#[test]
fn test_transaction_rollback() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
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
                panic!("{err}");
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
                panic!("{err}");
            }
        }
    }

    assert_eq!(
        db.select_all("SELECT count(*) FROM t", &[], |row| row.get::<_, i64>(0))
            .unwrap()
            .first()
            .unwrap(),
        &0
    );
}

#[test]
fn test_uncommitted_transaction() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
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
        db.select_all("SELECT count(*) FROM t", &[], |row| row.get::<_, i64>(0))
            .unwrap()
            .first()
            .unwrap(),
        &0
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

fn select_regex<T, P>(db: &SQLite3Driver, text: T, pattern: P, whole_word: bool, case_sensitive: bool) -> i64
where
    T: Into<Literal>,
    P: Into<Literal>,
{
    *db.select_all(
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
    let db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    // Default
    assert_eq!(select_regex(&db, "abcd", "BC", false, false), 1);
    assert_eq!(select_regex(&db, "abcd", "BB", false, false), 0);

    // Whole word
    assert_eq!(select_regex(&db, "abc d", "ABC", true, false), 1);
    assert_eq!(select_regex(&db, "a bc d", "BC", true, false), 1);
    assert_eq!(select_regex(&db, "a bcd", "BCD", true, false), 1);
    assert_eq!(select_regex(&db, "abcd", "ABCD", true, false), 1);
    assert_eq!(select_regex(&db, "abcd", "ABC", true, false), 0);
    assert_eq!(select_regex(&db, "abcd", "", true, false), 0);

    // Case-sensitive
    assert_eq!(select_regex(&db, "abcd", "bc", false, true), 1);
    assert_eq!(select_regex(&db, "abcd", "BC", false, true), 0);

    // Whole word & case-sensitive
    assert_eq!(select_regex(&db, "a bc d", "bc", true, true), 1);
    assert_eq!(select_regex(&db, "abcd", "ABC", true, true), 0);
    assert_eq!(select_regex(&db, "abcd", "abc", true, true), 0);

    // Escape sequences, case-insensitive
    assert_eq!(select_regex(&db, "abcd", "\\w+", false, false), 1);
    assert_eq!(select_regex(&db, "abcd", "\\W+", false, false), 0);
    assert_eq!(select_regex(&db, "....", "\\w+", false, false), 0);
    assert_eq!(select_regex(&db, "....", "\\W+", false, false), 1);

    // Escape sequences, case-sensitive
    assert_eq!(select_regex(&db, "abcd", "\\w+", false, true), 1);
    assert_eq!(select_regex(&db, "abcd", "\\W+", false, true), 0);
    assert_eq!(select_regex(&db, "....", "\\w+", false, true), 0);
    assert_eq!(select_regex(&db, "....", "\\W+", false, true), 1);

    // Regex, number
    assert_eq!(select_regex(&db, 123, "123", false, true), 1);
    assert_eq!(select_regex(&db, 123, "\\d+", false, true), 1);
    assert_eq!(select_regex(&db, 123, "\\d+1", false, true), 0);

    // NULL
    assert_eq!(select_regex(&db, None::<i64>, "NULL", true, true), 1);
    assert_eq!(select_regex(&db, None::<i64>, "null", true, false), 1);

    // REAL
    assert_eq!(select_regex(&db, 1.23, "1.23", true, true), 1);
    assert_eq!(select_regex(&db, 1.23, "1.23", true, false), 1);
    assert_eq!(select_regex(&db, 1.23, "1.24", true, false), 0);

    // Invalid parameter types
    assert_eq!(select_regex(&db, "abcd", 0, false, false), 0);
}

fn handle(db: &mut SQLite3Driver, query: &str, params: &[Literal], mode: QueryMode) -> String {
    let mut w = vec![];
    db.handle(&mut w, query, params, mode).unwrap();
    regex::Regex::new(r#""time":[\d.]+"#)
        .unwrap()
        .replace(&read_msgpack_into_json(std::io::Cursor::new(w)), r#""time":0"#)
        .to_string()
}

#[test]
fn test_handle_select() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert_eq!(
        handle(&mut db, "SELECT 1, 2", &[], QueryMode::ReadOnly),
        r#"{"records":{"1":[1],"2":[2]},"warnings":[],"time":0}"#
    );
}

#[test]
fn test_handle_table_schema() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
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
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    assert!(db
        .handle(
            &mut vec![],
            "EDITOR_PRAGMA table_schema",
            &[1.into(), 2.into()],
            QueryMode::ReadOnly,
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
        )
        .unwrap_err()
        .to_string()
        .contains("invalid argument"));
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn test_handle_load_extensions() {
    // sudo apt install -y libsqlite3-mod-spatialite
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    handle(
        &mut db,
        "EDITOR_PRAGMA load_extensions",
        &["mod_spatialite".into()],
        QueryMode::ReadOnly,
    );
}

#[test]
fn test_invalid_utf8_text() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    let mut warnings = vec![];
    db.execute("SELECT CAST(x'ff' AS TEXT)", &[], ExecMode::ReadWrite, &mut warnings)
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
fn test_invalid_utf8_table_name() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();
    let mut warnings = vec![];
    let mut query = "CREATE TABLE ab(x)".as_bytes().to_vec();
    query[14] = 255;
    let query = unsafe { String::from_utf8_unchecked(query) };
    db.execute(&query, &[], ExecMode::ReadWrite, &mut warnings).unwrap();
    assert_eq!(
        db.list_tables(false).unwrap(),
        (
            vec![Table {
                database: Rc::new("main".to_owned()),
                name: Rc::new("a�".to_owned()),
                type_: TableType::Table,
                column_names: vec!["x".to_owned()],
            }],
            vec![
                InvalidUTF8 {
                    text_lossy: "a�".to_owned(),
                    bytes: "61ff".to_owned(),
                    context: Some("pragma_table_list.name".to_owned()),
                },
                InvalidUTF8 {
                    text_lossy: "a�".to_owned(),
                    bytes: "61ff".to_owned(),
                    context: Some("list_columns.table_name".to_owned())
                },
            ],
        )
    );
}

#[cfg(feature = "sqlcipher")]
#[test]
fn test_sqlcipher() {
    let f = tempfile::NamedTempFile::new().unwrap();
    let path = &f.path().to_string_lossy();
    {
        let mut db = SQLite3Driver::connect(path, false, &Some("key1")).unwrap();
        db.execute(&"CREATE TABLE t(x)", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
    }
    {
        let mut db = SQLite3Driver::connect(path, false, &Some("key2")).unwrap();
        db.execute(&"SELECT * FROM t", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap_err();
    }
    {
        let mut db = SQLite3Driver::connect(path, false, &Some("key1")).unwrap();
        db.execute(&"SELECT * FROM t", &[], ExecMode::ReadWrite, &mut vec![])
            .unwrap();
    }
}

#[test]
fn test_cache_clear() {
    let mut db = SQLite3Driver::connect(":memory:", false, &None::<&str>).unwrap();

    // initialize the data_version
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "SELECT ?",
        &[1.into()],
        QueryMode::ReadOnly,
    )
    .unwrap();

    let count = db.pager().cache_clear_count;

    // readonly
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "SELECT ?",
        &[1.into()],
        QueryMode::ReadOnly,
    )
    .unwrap();
    assert_eq!(db.pager().cache_clear_count, count);

    // readwrite
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "CREATE TABLE t(x)",
        &[],
        QueryMode::ReadWrite,
    )
    .unwrap();
    assert_eq!(db.pager().cache_clear_count, count + 1);

    // script
    db.handle(
        &mut Cursor::new(Vec::<u8>::new()),
        "CREATE TABLE u(x)",
        &[],
        QueryMode::Script,
    )
    .unwrap();
    assert_eq!(db.pager().cache_clear_count, count + 2);
}
