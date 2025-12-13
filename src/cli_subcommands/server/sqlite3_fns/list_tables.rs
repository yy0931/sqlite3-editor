use std::collections::HashMap;
use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::column_origin::column_origin;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::sqlite_escape::escape_sql_identifier;
use crate::utf8_extractor::get_utf8_string;
use crate::utf8_extractor::InvalidUTF8;

pub fn list_tables(conn: &rusqlite::Connection) -> std::result::Result<(TableList, Vec<InvalidUTF8>), CLIError> {
    let mut warnings = vec![];

    // List tables in the main database excluding the sqlite_ tables
    let mut table_list = select_all(
        conn,
        r#"SELECT schema, name, type FROM pragma_table_list WHERE NOT (name LIKE "sqlite\_%" ESCAPE "\") AND schema = 'main' COLLATE NOCASE"#,
        &[],
        |row| {
            Ok(TableNameAndColumns {
                database: Rc::new(get_utf8_string(row, 0, |err| warnings.push(err.with("pragma_table_list.schema")))?),
                name: Rc::new(get_utf8_string(row, 1, |err| warnings.push(err.with("pragma_table_list.name")))?),
                type_: TableType::from(
                    get_utf8_string(row, 2, |err| warnings.push(err.with("pragma_table_list.type (list_tables)")))?.as_str(),
                ),
                column_names: vec![],
            })
        },
    )?;

    // List columns
    // Ignore broken tables
    {
        let mut column_names_map = HashMap::<String, Vec<String>>::new();

        let _ = select_all(
            conn,
            r#"
WITH tables AS (
    SELECT DISTINCT name AS "table_name"
    FROM pragma_table_list()
    WHERE schema = 'main')
SELECT table_name, p.name
FROM tables
JOIN main.pragma_table_info("table_name") p"#,
            &[],
            |row| {
                column_names_map
                    .entry(get_utf8_string(row, 0, |err| warnings.push(err.with("list_columns.table_name")))?)
                    .or_default()
                    .push(get_utf8_string(row, 1, |err| warnings.push(err.with("list_columns.column_name")))?);
                Ok(0)
            },
        );

        for table in &mut table_list {
            if let Some(columns) = column_names_map.remove(table.name.as_ref()) {
                for column in columns {
                    table.column_names.push(column);
                }
            }
        }
    }

    // List foreign keys
    let mut entity_relationships = select_all(
        conn,
        r#"SELECT t.name, f."table", f."from" FROM pragma_table_list t INNER JOIN pragma_foreign_key_list(name) f WHERE t.schema = 'main' COLLATE NOCASE AND NOT (t.name LIKE "sqlite\_%" ESCAPE "\");"#,
        &[],
        |row| {
            Ok(EntityRelationship {
                source: get_utf8_string(row, 0, |err| warnings.push(err.with("pragma_table_list.name")))?,
                target: get_utf8_string(row, 1, |err| warnings.push(err.with("pragma_table_list.table")))?,
                source_column: get_utf8_string(row, 2, |err| warnings.push(err.with("pragma_table_list.from")))?,
            })
        },
    )?;

    // List column origins of views
    if let Ok(views) = select_all(
        conn,
        "SELECT name FROM pragma_table_list WHERE schema = 'main' AND type = 'view'",
        &[],
        |row| get_utf8_string(row, 0, |err| warnings.push(err.with("pragma_table_list.name"))),
    ) {
        for view_name in views {
            let Ok(column_origins) = column_origin(
                unsafe { conn.handle() },
                &format!("SELECT * FROM {} LIMIT 0", escape_sql_identifier(&view_name)),
            ) else {
                continue;
            };
            for (column, origin) in column_origins {
                if origin.database.to_lowercase() != "main" {
                    continue;
                }
                entity_relationships.push(EntityRelationship {
                    source: view_name.clone(),
                    target: origin.table,
                    source_column: column,
                })
            }
        }
    }

    // List views
    let views = select_all(
        conn,
        // `sql IS NOT NULL` may not be needed
        "SELECT name, sql FROM sqlite_schema WHERE type = 'view' AND sql IS NOT NULL",
        &[],
        |row| {
            Ok((
                get_utf8_string(row, 0, |err| warnings.push(err.with("sqlite_schema.name")))?,
                get_utf8_string(row, 1, |err| warnings.push(err.with("sqlite_schema.sql")))?,
            ))
        },
    )?;

    Ok((
            TableList {
                table_list,
                entity_relationships,
                views,
                virtual_tables: select_all(conn,"WITH virutal_tables AS (SELECT name FROM pragma_table_list WHERE schema = 'main' AND type = 'virtual') SELECT name, sql FROM sqlite_schema s WHERE s.name IN virutal_tables", &[], |row| Ok((
                    get_utf8_string(row, 0, |err| warnings.push(err.with("table_schema.name")))?,
                    get_utf8_string(row, 1, |err| warnings.push(err.with("table_schema.sql")))?,
                )))?,
            },
            warnings,
        ))
}

#[derive(Clone, Debug, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableList {
    pub table_list: Vec<TableNameAndColumns>,
    pub entity_relationships: Vec<EntityRelationship>,
    pub views: Vec<(String, String)>,
    pub virtual_tables: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TableNameAndColumns {
    pub database: Rc<String>,
    pub name: Rc<String>,
    #[serde(rename = "type")]
    pub type_: TableType,
    pub column_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct EntityRelationship {
    pub source: String,
    pub target: String,
    pub source_column: String,
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::rc::Rc;

    use crate::cli_subcommands::server::sqlite3_fns::list_tables::list_tables;
    use crate::cli_subcommands::server::sqlite3_fns::list_tables::EntityRelationship;
    use crate::cli_subcommands::server::sqlite3_fns::list_tables::TableNameAndColumns;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
    use crate::utf8_extractor::InvalidUTF8;

    #[test]
    fn test_entity_relationships() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "
CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT;
CREATE TABLE v(x INTEGER NOT NULL REFERENCES t);",
        )
        .unwrap();
        assert_eq!(
            list_tables(&db).unwrap().0.entity_relationships,
            vec![EntityRelationship {
                source: "v".to_owned(),
                target: "t".to_owned(),
                source_column: "x".to_owned(),
            }],
        );
    }

    #[test]
    fn test_list_tables() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t1(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT;
        CREATE TABLE t2(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT;
        CREATE TEMP TABLE t3(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        )
        .unwrap();
        assert_eq!(
            list_tables(&db)
                .unwrap()
                .0
                .table_list
                .into_iter()
                .collect::<HashSet<TableNameAndColumns>>(),
            HashSet::from([
                TableNameAndColumns {
                    database: Rc::new("main".to_owned()),
                    name: Rc::new("t1".to_owned()),
                    type_: TableType::Table,
                    column_names: vec!["x".to_owned()],
                },
                TableNameAndColumns {
                    database: Rc::new("main".to_owned()),
                    name: Rc::new("t2".to_owned()),
                    type_: TableType::Table,
                    column_names: vec!["x".to_owned()],
                },
            ]),
        );
    }

    #[test]
    fn test_list_views() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE VIEW v1 AS SELECT 1;
        CREATE VIEW v2 AS SELECT 2;",
        )
        .unwrap();
        assert_eq!(
            list_tables(&db).unwrap().0.views.into_iter().collect::<HashSet<(String, String)>>(),
            HashSet::from([
                ("v1".to_owned(), "CREATE VIEW v1 AS SELECT 1".to_owned()),
                ("v2".to_owned(), "CREATE VIEW v2 AS SELECT 2".to_owned()),
            ]),
        );
    }

    #[test]
    fn test_invalid_utf8_table_name() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let mut query = "CREATE TABLE ab(x)".as_bytes().to_vec();
        query[14] = 255;
        let query = unsafe { String::from_utf8_unchecked(query) };
        db.execute(&query, ()).unwrap();
        let result = list_tables(&db).unwrap();
        assert_eq!(
            result.0.table_list,
            vec![TableNameAndColumns {
                database: Rc::new("main".to_owned()),
                name: Rc::new("a�".to_owned()),
                type_: TableType::Table,
                column_names: vec!["x".to_owned()],
            }],
        );
        assert_eq!(
            result.1,
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
        );
    }
}
