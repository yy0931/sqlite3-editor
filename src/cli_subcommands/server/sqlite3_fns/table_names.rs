use std::rc::Rc;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableName;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::utf8_extractor::get_utf8_string;
use crate::utf8_extractor::InvalidUTF8;

pub fn table_names(conn: &rusqlite::Connection) -> std::result::Result<(Vec<TableName>, Vec<InvalidUTF8>), CLIError> {
    let mut warnings = vec![];

    // list tables in all databases including sqlite_ tables
    let tables = select_all(
        conn,
        r#"SELECT schema, name, type FROM pragma_table_list"#,
        &[],
        |row| {
            Ok(TableName {
                database: Rc::new(get_utf8_string(row, 0, |err| {
                    warnings.push(err.with("pragma_table_list.schema"))
                })?),
                name: Rc::new(get_utf8_string(row, 1, |err| {
                    warnings.push(err.with("pragma_table_list.name"))
                })?),
                type_: TableType::from(
                    get_utf8_string(row, 2, |err| {
                        warnings.push(err.with("pragma_table_list.type (list_tables)"))
                    })?
                    .as_str(),
                ),
            })
        },
    )?;

    Ok((tables, warnings))
}

#[cfg(test)]
mod test {
    use std::collections::HashSet;
    use std::rc::Rc;

    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableName;
    use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
    use crate::cli_subcommands::server::sqlite3_fns::table_names::table_names;

    #[test]
    fn test_table_names() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE t1(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT;
            CREATE TABLE t2(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT;
            CREATE TEMP TABLE t3(x INTEGER NOT NULL PRIMARY KEY) WITHOUT ROWID, STRICT",
        )
        .unwrap();
        assert_eq!(
            table_names(&db).unwrap().0.into_iter().collect::<HashSet<TableName>>(),
            HashSet::from([
                TableName {
                    database: Rc::new("main".to_owned()),
                    name: Rc::new("t1".to_owned()),
                    type_: TableType::Table,
                },
                TableName {
                    database: Rc::new("main".to_owned()),
                    name: Rc::new("t2".to_owned()),
                    type_: TableType::Table,
                },
                TableName {
                    database: Rc::new("temp".to_owned()),
                    name: Rc::new("t3".to_owned()),
                    type_: TableType::Table,
                },
                TableName {
                    database: Rc::new("temp".to_owned()),
                    name: Rc::new("sqlite_temp_schema".to_owned()),
                    type_: TableType::Table,
                },
                TableName {
                    database: Rc::new("main".to_owned()),
                    name: Rc::new("sqlite_schema".to_owned()),
                    type_: TableType::Table,
                },
            ]),
        );
    }
}
