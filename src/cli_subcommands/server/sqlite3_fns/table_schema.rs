use std::collections::HashMap;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::column_origin::column_origin;
use crate::cli_subcommands::server::sqlite3_fns::get_foreign_keys::ForeignKeyList;
use crate::cli_subcommands::server::sqlite3_fns::get_foreign_keys::ForeignKeyListCache;
use crate::cli_subcommands::server::sqlite3_fns::is_rowid::is_rowid;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::ColumnOriginAndIsRowId;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::IndexColumn;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchema;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumn;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumnForeignKey;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaIndex;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaTrigger;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::cli_value::CLIValue;
use crate::sqlite_escape::escape_sql_identifier;
use crate::utf8_extractor::get_utf8_string;
use crate::utf8_extractor::get_utf8_string_optional;
use crate::utf8_extractor::InvalidUTF8;

/// Collect table definitions from sqlite_schema, pragma_table_list, pragma_foreign_key_list, pragma_table_xinfo, pragma_index_list, and pragm_index_info.
pub fn table_schema(
    conn: &rusqlite::Connection,
    database: &str,
    table_name: &str,
) -> std::result::Result<(Option<TableSchema>, Vec<InvalidUTF8>), CLIError> {
    let mut warnings = vec![];

    // Select pragma_table_list
    let (table_type, wr, strict) = {
        let all = select_all(
            conn,
            "SELECT type, wr, strict FROM pragma_table_list WHERE schema = ? COLLATE NOCASE AND name = ? COLLATE NOCASE",
            &[database.into(), table_name.into()],
            |row| {
                Ok((
                    get_utf8_string(row, 0, |err| warnings.push(err.with("pragma_table_list.type (table_schema)")))?,
                    row.get::<_, i64>(1)? != 0,
                    row.get::<_, i64>(2)? != 0,
                ))
            },
        )?;
        let Some(one) = all.first() else {
            return Ok((None, warnings));
        };
        one.to_owned()
    };

    let table_type = TableType::from(table_type.as_str());

    // rowid tables https://www.sqlite.org/rowidtable.html or virtual tables without `WITHOUT ROWID` https://www.sqlite.org/vtab.html#_without_rowid_virtual_tables_
    let has_rowid_column = (table_type == TableType::Table || table_type == TableType::Virtual) && !wr;

    // Select pragma_foreign_key_list
    let mut foreign_key_list_cache = ForeignKeyListCache::default();

    let (column_origins, foreign_keys): (Option<HashMap<String, ColumnOriginAndIsRowId>>, ForeignKeyList) = if table_type == TableType::View
    {
        let column_origins = column_origin(
            unsafe { conn.handle() },
            &format!("SELECT * FROM {} LIMIT 0", escape_sql_identifier(table_name)),
        )
        .unwrap_or_default();

        // In this case:
        // ```
        // CREATE TABLE t1(x INTEGER PRIMARY KEY);
        // CREATE TABLE t2(y INTEGER REFERENCES t1(x));
        // CREATE VIEW table_name AS SELECT y as z FROM t2;
        // ```
        // column_origins = {"z": ("main", "t2", "y")}
        // origin_fk = { from: "y", to: "x", table: "t1" }
        let mut foreign_keys = HashMap::<String, Vec<TableSchemaColumnForeignKey>>::new();
        for (from, to) in &column_origins {
            if let Some(origin_fk) = foreign_key_list_cache
                .get(conn, &to.database, &to.table, &mut warnings)?
                .get(&to.column)
            {
                let vec = foreign_keys.entry(from.to_owned()).or_default();
                for item in origin_fk.iter() {
                    vec.push(item.to_owned());
                }
            }
        }
        (
            Some(
                column_origins
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            ColumnOriginAndIsRowId::new(is_rowid(conn, &v, &mut warnings).unwrap_or(false /* TODO: error handling */), v),
                        )
                    })
                    .collect::<HashMap<String, ColumnOriginAndIsRowId>>(),
            ),
            foreign_keys,
        )
    } else {
        (None, foreign_key_list_cache.get(conn, database, table_name, &mut warnings)?.clone())
    };

    // Select sqlite_sequence
    // NOTE: There is no way to check if an empty table has an autoincrement column.
    let has_table_auto_increment_column: bool = !select_all(
        conn,
        &format!(
            "SELECT name FROM {}.sqlite_schema WHERE type = 'table' AND name = 'sqlite_sequence'",
            escape_sql_identifier(database)
        ),
        &[],
        |_row| Ok(()),
    )?
    .is_empty()
        && !select_all(
            conn,
            &format!(
                "SELECT * FROM {}.sqlite_sequence WHERE name = ? COLLATE NOCASE",
                escape_sql_identifier(database)
            ),
            &[table_name.into()],
            |_row| Ok(()),
        )?
        .is_empty();

    let get_sql_column = |records: Option<Vec<(Option<std::string::String>,)>>| -> Option<String> {
        if let Some(mut records) = records {
            if !records.is_empty() {
                return std::mem::take(&mut records[0].0);
            }
        }
        None
    };

    // Select pragma_table_xinfo
    let columns: Vec<TableSchemaColumn> = select_all(
        conn,
        &format!(
            "PRAGMA {}.table_xinfo({})",
            escape_sql_identifier(database),
            escape_sql_identifier(table_name)
        ),
        &[],
        |row| {
            let name = get_utf8_string(row, 1, |err| warnings.push(err.with("table_xinfo.name")))?;
            let pk = row.get::<_, i64>(5)? != 0;

            Ok(TableSchemaColumn {
                cid: row.get::<_, i64>(0)?,
                notnull: row.get::<_, i64>(3)? != 0,
                type_: if table_type == TableType::View {
                    // NOTE: Why does table_xinfo always return "BLOB" for views?
                    "".to_owned()
                } else {
                    get_utf8_string(row, 2, |err| warnings.push(err.with("table_xinfo.type")))?
                },
                pk,
                auto_increment: pk && has_table_auto_increment_column,
                foreign_keys: foreign_keys.get(&name).cloned().unwrap_or_default(),
                hidden: row.get::<_, i64>(6)?,
                dflt_value: get_utf8_string_optional(row, 4, |err| warnings.push(err.with("table_xinfo.dflt_value")))
                    .expect("Unexpected value for table_xinfo.dflt_value"),
                name,
            })
        },
    )?;

    // Select pragma_index_list
    let mut indexes: Vec<TableSchemaIndex> = select_all(
        conn,
        &format!(
            "PRAGMA {}.index_list({})",
            escape_sql_identifier(database),
            escape_sql_identifier(table_name)
        ),
        &[],
        |row| {
            let name = get_utf8_string(row, 1, |err| warnings.push(err.with("index_list.name")))?;
            Ok(TableSchemaIndex {
                seq: row.get::<_, i64>(0)?,
                unique: row.get::<_, i64>(2)?,
                origin: get_utf8_string(row, 3, |err| warnings.push(err.with("index_list.origin")))?,
                partial: row.get::<_, i64>(4)?,
                schema: get_sql_column(
                    select_all(
                        conn,
                        &format!(
                            "SELECT sql FROM {}.sqlite_schema WHERE type = 'index' AND name = ? COLLATE NOCASE",
                            escape_sql_identifier(database)
                        ),
                        &[CLIValue::String(name.to_owned())],
                        |row| Ok((get_utf8_string(row, 0, |err| warnings.push(err.with("sqlite_schema.sql"))).ok(),)),
                    )
                    .ok(),
                ),
                columns: vec![], // this will be replaced
                name,
            })
        },
    )?;

    // List indexes
    for index in &mut indexes {
        index.columns = select_all(
            conn,
            &format!("PRAGMA index_info({})", escape_sql_identifier(&index.name)),
            &[],
            |row| {
                Ok(IndexColumn {
                    seqno: row.get::<_, i64>(0)?,
                    cid: row.get::<_, i64>(1)?,
                    name: get_utf8_string_optional(row, 2, |err| warnings.push(err.with("index_info.name")))?,
                })
            },
        )?;
    }

    // Get the table schema
    let schema = get_sql_column(
        select_all(
            conn,
            &format!(
                "SELECT sql FROM {}.sqlite_schema WHERE name = ? COLLATE NOCASE",
                escape_sql_identifier(database)
            ),
            &[table_name.into()],
            |row| Ok((get_utf8_string(row, 0, |err| warnings.push(err.with("sqlite_schema.sql (table)"))).ok(),)),
        )
        .ok(),
    )
    .unwrap_or_else(|| "".to_string());

    // List triggers
    let triggers: Vec<TableSchemaTrigger> = select_all(
        conn,
        &format!(
            "SELECT name, sql FROM {}.sqlite_schema WHERE tbl_name = ? AND type = 'trigger'",
            escape_sql_identifier(database)
        ),
        &[table_name.into()],
        |row| {
            Ok(TableSchemaTrigger {
                name: get_utf8_string(row, 0, |err| warnings.push(err.with("sqlite_schema.name (trigger)")))?,
                sql: get_utf8_string(row, 1, |err| warnings.push(err.with("sqlite_schema.sql (trigger)")))?,
            })
        },
    )?;

    Ok((
        Some(TableSchema {
            name: Some(table_name.to_string()),
            schema: Some(schema),
            has_rowid_column,
            strict,
            columns,
            indexes,
            triggers,
            custom_query: None,
            column_origins,
            type_: table_type,
        }),
        warnings,
    ))
}

#[cfg(test)]
mod test_table_schema {
    use std::collections::HashMap;

    use crate::cli_subcommands::server::sqlite3_fns::query_schema::query_schema;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::ColumnOriginAndIsRowId;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::IndexColumn;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchema;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumn;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumnForeignKey;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaIndex;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaTrigger;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
    use crate::cli_subcommands::server::sqlite3_fns::table_schema::table_schema;

    #[test]
    fn test_table() {
        let db = rusqlite::Connection::open_in_memory().unwrap();

        db.execute_batch("CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT").unwrap();

        assert_eq!(
            table_schema(&db, "main", "t").unwrap().0.unwrap(),
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
                    foreign_keys: vec![],
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
        let db = rusqlite::Connection::open_in_memory().unwrap();

        db.execute_batch(
            "CREATE TABLE t(x);
            CREATE VIEW u AS SELECT x as y FROM t;",
        )
        .unwrap();

        assert_eq!(
            table_schema(&db, "main", "u").unwrap().0.unwrap(),
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
                    foreign_keys: vec![],
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
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT;
            CREATE TABLE u(y INTEGER NOT NULL REFERENCES t(x));",
        )
        .unwrap();
        assert_eq!(
            table_schema(&db, "main", "u").unwrap().0.unwrap().columns[0].foreign_keys[0],
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
    fn test_foreign_key_shorthand() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT;
            CREATE TABLE u(x INTEGER NOT NULL REFERENCES t);",
        )
        .unwrap();
        assert_eq!(
            table_schema(&db, "main", "u").unwrap().0.unwrap().columns[0].foreign_keys,
            vec![TableSchemaColumnForeignKey {
                id: 0,
                seq: 0,
                table: "t".to_owned(),
                to: "x".to_owned(),
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned(),
            }],
        );
    }

    #[test]
    fn test_foreign_key_shorthand_multi_column() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t(a, b, PRIMARY KEY (b, a));
            CREATE TABLE u(x, y, FOREIGN KEY (x, y) REFERENCES t);",
        )
        .unwrap();
        let columns = table_schema(&db, "main", "u").unwrap().0.unwrap().columns;
        assert_eq!(
            columns[0].foreign_keys,
            vec![TableSchemaColumnForeignKey {
                id: 0,
                seq: 0,
                table: "t".to_owned(),
                to: "b".to_owned(),
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned(),
            }],
        );
        assert_eq!(
            columns[1].foreign_keys,
            vec![TableSchemaColumnForeignKey {
                id: 0,
                seq: 1,
                table: "t".to_owned(),
                to: "a".to_owned(),
                on_update: "NO ACTION".to_owned(),
                on_delete: "NO ACTION".to_owned(),
                match_: "NONE".to_owned(),
            }],
        );
    }

    #[test]
    fn test_auto_increment() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT);
            INSERT INTO t DEFAULT VALUES",
        )
        .unwrap();
        assert!(table_schema(&db, "main", "t").unwrap().0.unwrap().columns[0].auto_increment);
    }

    #[test]
    fn test_not_auto_increment() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY);
            INSERT INTO t DEFAULT VALUES;",
        )
        .unwrap();
        assert!(!table_schema(&db, "main", "t").unwrap().0.unwrap().columns[0].auto_increment);
    }

    #[test]
    fn test_default_value() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE t(a DEFAULT NULL, b DEFAULT 1, c DEFAULT 1.2, d DEFAULT 'a', e DEFAULT x'1234', f DEFAULT (1 + 2))")
            .unwrap();
        assert_eq!(
            table_schema(&db, "main", "t")
                .unwrap()
                .0
                .unwrap()
                .columns
                .into_iter()
                .map(|v| v.dflt_value)
                .collect::<Vec<_>>(),
            vec![
                Some("NULL".to_owned()),
                Some("1".to_owned()),
                Some("1.2".to_owned()),
                Some("'a'".to_owned()),
                Some("x'1234'".to_owned()),
                Some("1 + 2".to_owned()),
            ]
        );
    }

    #[test]
    fn test_index() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t(x UNIQUE);
            CREATE INDEX idx1 ON t(x);",
        )
        .unwrap();
        assert_eq!(
            table_schema(&db, "main", "t").unwrap().0.unwrap().indexes,
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
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE t(x)").unwrap();
        let sql = "CREATE TRIGGER trigger_insert AFTER INSERT ON t BEGIN SELECT 1; END";
        db.execute(sql, ()).unwrap();
        assert_eq!(
            table_schema(&db, "main", "t").unwrap().0.unwrap().triggers,
            vec![TableSchemaTrigger {
                name: "trigger_insert".to_owned(),
                sql: sql.to_owned(),
            },]
        );
    }

    #[test]
    fn test_query_schema() {
        let db = rusqlite::Connection::open_in_memory().unwrap();

        db.execute_batch("CREATE TABLE t(x INTEGER)").unwrap();
        assert_eq!(
            query_schema(&db, "SELECT 1, x FROM t").unwrap().0,
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
                        foreign_keys: vec![],
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
                        foreign_keys: vec![],
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
        let db = rusqlite::Connection::open_in_memory().unwrap();

        db.execute_batch(
            "CREATE TABLE t(x);
            CREATE TEMP TABLE t(y);
            CREATE TEMP TABLE u(y);",
        )
        .unwrap();

        assert_eq!(
            table_schema(&db, "main", "t").unwrap().0.unwrap().schema.unwrap(),
            "CREATE TABLE t(x)"
        );
        assert_eq!(
            table_schema(&db, "temp", "t").unwrap().0.unwrap().schema.unwrap(),
            "CREATE TABLE t(y)"
        );
        assert_eq!(
            table_schema(&db, "temp", "u").unwrap().0.unwrap().schema.unwrap(),
            "CREATE TABLE u(y)"
        );
    }

    mod test_indirect_foreign_key {
        use std::collections::HashMap;

        use crate::cli_subcommands::server::sqlite3_fns::query_schema::query_schema;
        use crate::cli_subcommands::server::sqlite3_fns::schema_types::ColumnOriginAndIsRowId;
        use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumnForeignKey;
        use crate::cli_subcommands::server::sqlite3_fns::table_schema::table_schema;

        #[test]
        fn test_table_schema() {
            let db = rusqlite::Connection::open_in_memory().unwrap();

            db.execute_batch(
                "CREATE TABLE t1(x INTEGER PRIMARY KEY);
                CREATE TABLE t2(y INTEGER REFERENCES t1(x));
                CREATE VIEW table_name AS SELECT y as z FROM t2;",
            )
            .unwrap();
            let schema = table_schema(&db, "main", "table_name").unwrap().0.unwrap();
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
                vec![TableSchemaColumnForeignKey {
                    id: 0,
                    seq: 0,
                    table: "t1".to_owned(),
                    to: "x".to_owned(),
                    on_update: "NO ACTION".to_owned(),
                    on_delete: "NO ACTION".to_owned(),
                    match_: "NONE".to_owned(),
                }]
            );
        }

        #[test]
        fn test_query_schema() {
            let db = rusqlite::Connection::open_in_memory().unwrap();

            db.execute_batch(
                "CREATE TABLE t1(x INTEGER PRIMARY KEY);
                CREATE TABLE t2(y INTEGER REFERENCES t1(x));
                CREATE VIEW table_name AS SELECT y as z FROM t2;",
            )
            .unwrap();
            let schema = query_schema(&db, "SELECT * FROM table_name").unwrap().0;
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
                vec![TableSchemaColumnForeignKey {
                    id: 0,
                    seq: 0,
                    table: "t1".to_owned(),
                    to: "x".to_owned(),
                    on_update: "NO ACTION".to_owned(),
                    on_delete: "NO ACTION".to_owned(),
                    match_: "NONE".to_owned(),
                }]
            );
        }
    }

    mod test_rowid_alias {
        use crate::cli_subcommands::server::sqlite3_fns::column_origin::ColumnOrigin;
        use crate::cli_subcommands::server::sqlite3_fns::is_rowid::is_rowid;
        use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
        use crate::sqlite_escape::escape_sql_identifier;
        use crate::utf8_extractor::get_utf8_string;

        fn check_is_rowid(db: &rusqlite::Connection, table: &str, column: &str, expected: bool) {
            assert_eq!(
                is_rowid(db, &ColumnOrigin::new("main", table, column), &mut vec![]).unwrap(),
                expected
            );
        }

        fn check_is_alias_to_rowid(db: &rusqlite::Connection, table: &str, column: &str, yes: bool) {
            db.execute_batch(&format!("INSERT INTO {table} DEFAULT VALUES")).unwrap();
            assert_eq!(
                select_all(
                    db,
                    &format!(
                        "SELECT typeof({}) FROM {}",
                        escape_sql_identifier(column),
                        escape_sql_identifier(table)
                    ),
                    &[],
                    |row| { get_utf8_string(row, 0, |_| {}) }
                )
                .unwrap()
                .first()
                .unwrap(),
                if yes { "integer" } else { "null" }
            );
        }

        #[test]
        fn test_single_integer_pk() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(x INTEGER PRIMARY KEY)").unwrap();
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&db, "t", "x", true);
        }

        #[test]
        fn test_single_integer_not_null_pk() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY)").unwrap();
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&db, "t", "x", true);
        }

        #[test]
        fn test_integer_unique_pk() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(x INTEGER UNIQUE PRIMARY KEY)").unwrap();
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&db, "t", "x", true);
        }

        #[test]
        fn test_multiple_integer_pk() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(x INTEGER, y INTEGER, PRIMARY KEY (x, y))")
                .unwrap();
            check_is_rowid(&db, "t", "x", false);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&db, "t", "x", false);
        }

        #[test]
        fn test_int_pk() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(x INT PRIMARY KEY)").unwrap();
            check_is_rowid(&db, "t", "x", false);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&db, "t", "x", false);
        }

        #[test]
        fn test_lowercase_integer_pk() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(x integer primary key)").unwrap();
            check_is_rowid(&db, "t", "x", true);
            check_is_rowid(&db, "t", "rowid", true);
            check_is_alias_to_rowid(&db, "t", "x", true);
        }

        #[test]
        fn test_shadowed_rowid() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(rowid TEXT)").unwrap();
            check_is_rowid(&db, "t", "rowid", false);
            check_is_rowid(&db, "t", "_rowid_", true);
            check_is_alias_to_rowid(&db, "t", "rowid", false);
        }

        #[test]
        fn test_shadowed_rowid_and_oid() {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE t(rowid TEXT, oid TEXT)").unwrap();
            check_is_rowid(&db, "t", "rowid", false);
            check_is_rowid(&db, "t", "oid", false);
            check_is_rowid(&db, "t", "_rowid_", true);
            check_is_alias_to_rowid(&db, "t", "oid", false);
        }
    }
}
