use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::column_origin::ColumnOrigin;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::sqlite_escape::escape_sql_identifier;
use crate::utf8_extractor::get_utf8_string;
use crate::utf8_extractor::InvalidUTF8;

pub fn is_rowid(
    conn: &rusqlite::Connection,
    column_origin: &ColumnOrigin,
    warnings: &mut Vec<InvalidUTF8>,
) -> std::result::Result<bool, CLIError> {
    struct Column {
        name: String,
        type_: String,
        pk: i64,
    }

    let table_xinfo = select_all(
        conn,
        &format!(
            "PRAGMA {}.table_xinfo({})",
            escape_sql_identifier(&column_origin.database),
            escape_sql_identifier(&column_origin.table)
        ),
        &[],
        |row| {
            Ok(Column {
                name: get_utf8_string(row, 1, |err| warnings.push(err.with("is_rowid.table_xinfo.name")))?,
                type_: get_utf8_string(row, 2, |err| warnings.push(err.with("is_rowid.table_xinfo.type")))?,
                pk: row.get::<_, i64>(5)?,
            })
        },
    )?;

    // - column_origin.column is "rowid" and there isn't a user-defined column named "rowid".
    // - column_origin.column is "_rowid_" and there isn't a user-defined column named "_rowid_".
    // - column_origin.column is "oid" and there isn't a user-defined column named "oid".
    if column_origin.column.to_lowercase() == "rowid" && table_xinfo.iter().all(|v| v.name.to_lowercase() != "rowid")
        || column_origin.column.to_lowercase() == "_rowid_"
            && table_xinfo.iter().all(|v| v.name.to_lowercase() != "_rowid_")
        || column_origin.column.to_lowercase() == "oid" && table_xinfo.iter().all(|v| v.name.to_lowercase() != "oid")
    {
        return Ok(true);
    }

    // - column_origin.column is a INTEGER PRIMARY KEY, where "INTEGER" need to be case-insensitive exact match, and there aren't multiple primary keys in the table.
    // > In the exception, the INTEGER PRIMARY KEY becomes an alias for the rowid.
    // > https://www.sqlite.org/rowidtable.html
    // sqlite3_column_origin_name() returns the INTEGER PRIMARY KEY column when queried against rowid.
    if table_xinfo
        .iter()
        .find(|v| v.name.to_lowercase() == column_origin.column.to_lowercase())
        .is_some_and(|v| v.type_.to_lowercase() == "integer" && v.pk != 0)
        && table_xinfo.iter().filter(|v| v.pk != 0).count() == 1
    {
        return Ok(true);
    }

    Ok(false)
}
