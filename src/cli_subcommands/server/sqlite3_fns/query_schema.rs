use std::collections::HashMap;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::column_origin::column_origin;
use crate::cli_subcommands::server::sqlite3_fns::get_foreign_keys::ForeignKeyListCache;
use crate::cli_subcommands::server::sqlite3_fns::is_rowid::is_rowid;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::ColumnOriginAndIsRowId;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchema;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumn;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableType;
use crate::utf8_extractor::InvalidUTF8;

pub fn query_schema(
    conn: &rusqlite::Connection,
    query: &str,
) -> std::result::Result<(TableSchema, Vec<InvalidUTF8>), CLIError> {
    let mut warnings = vec![];

    let column_origins = column_origin(
        unsafe { conn.handle() },
        // \n is to handle line comments, e.g. query = "SELECT a FROM b -- comments"
        &format!("SELECT * FROM ({query}\n) LIMIT 0"),
    )
    .unwrap_or_default();

    let stmt_str = format!("SELECT * FROM ({query}\n) LIMIT 0");
    let mut stmt = conn
        .prepare(&stmt_str)
        .or_else(|err| CLIError::new_query_error(err, &stmt_str, &[]))?;

    // NOTE: We need to call `stmt.column_names()` after `.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53)
    let _ = stmt.raw_query().next();
    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut foreign_key_list_cache = ForeignKeyListCache::default();

    Ok((
        TableSchema {
            type_: TableType::CustomQuery,
            name: None,
            indexes: vec![],
            triggers: vec![],
            schema: None,
            has_rowid_column: false,
            strict: false,
            columns: column_names
                .into_iter()
                .enumerate()
                .map(|(i, name)| TableSchemaColumn {
                    cid: i as i64,
                    dflt_value: None,
                    name: name.to_owned(),
                    notnull: false,
                    type_: "".to_owned(),
                    pk: false,
                    auto_increment: false,
                    foreign_keys: column_origins
                        .get(&name)
                        .and_then(|origin| {
                            foreign_key_list_cache
                                .get(conn, &origin.database, &origin.table, &mut warnings)
                                .ok()
                                .and_then(|map| map.get(&origin.column).cloned())
                        })
                        .unwrap_or_default(),
                    hidden: 0,
                })
                .collect::<Vec<_>>(),
            custom_query: Some(query.to_owned()),
            column_origins: Some(
                column_origins
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            ColumnOriginAndIsRowId::new(
                                is_rowid(conn, &v, &mut warnings).unwrap_or(false /* TODO: error handling */),
                                v,
                            ),
                        )
                    })
                    .collect::<HashMap<String, ColumnOriginAndIsRowId>>(),
            ),
        },
        warnings,
    ))
}
