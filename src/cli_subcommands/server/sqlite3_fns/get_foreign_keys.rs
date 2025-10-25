use std::collections::HashMap;
use std::collections::HashSet;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::schema_types::TableSchemaColumnForeignKey;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::sqlite_escape::escape_sql_identifier;
use crate::utf8_extractor::get_utf8_string;
use crate::utf8_extractor::get_utf8_string_optional;
use crate::utf8_extractor::InvalidUTF8;

/// column -> foreign_key[]
pub type ForeignKeyList = HashMap<String, Vec<TableSchemaColumnForeignKey>>;

#[derive(Default)]
pub struct ForeignKeyListCache {
    /// (database, table) -> column -> foreign_key[]
    tables: HashMap<(String, String), ForeignKeyList>,
}

impl ForeignKeyListCache {
    pub fn get_primary_key(
        &self,
        conn: &rusqlite::Connection,
        table_name: &str,
        seq: i64,
        warnings: &mut Vec<InvalidUTF8>,
    ) -> std::result::Result<Option<String>, CLIError> {
        Ok(select_all(
            conn,
            "SELECT name FROM pragma_table_info(?) WHERE pk = ? + 1",
            &[table_name.into(), seq.into()],
            |row| get_utf8_string(row, 0, |err| warnings.push(err.with("pragma_table_list.from"))),
        )?
        .first()
        .cloned())
    }

    pub fn get(
        &mut self,
        conn: &rusqlite::Connection,
        database: &str,
        table_name: &str,
        warnings: &mut Vec<InvalidUTF8>,
    ) -> std::result::Result<&ForeignKeyList, CLIError> {
        let key = (database.to_owned(), table_name.to_owned());
        if !self.tables.contains_key(&key) {
            let mut foreign_key_list = select_all(
                conn,
                &format!(
                    "PRAGMA {}.foreign_key_list({})",
                    escape_sql_identifier(database),
                    escape_sql_identifier(table_name)
                ),
                &[],
                |row| {
                    Ok(ForeignKeyListEntry {
                        id: row.get::<_, i64>(0)?,
                        seq: row.get::<_, i64>(1)?,
                        table: get_utf8_string(row, 2, |err| warnings.push(err.with("foreign_key_list.table")))?,
                        from: get_utf8_string(row, 3, |err| warnings.push(err.with("foreign_key_list.from")))?,
                        to: get_utf8_string_optional(row, 4, |err| warnings.push(err.with("foreign_key_list.to")))?,
                        on_update: get_utf8_string(row, 5, |err| {
                            warnings.push(err.with("foreign_key_list.on_update"))
                        })?,
                        on_delete: get_utf8_string(row, 6, |err| {
                            warnings.push(err.with("foreign_key_list.on_delete"))
                        })?,
                        match_: get_utf8_string(row, 7, |err| warnings.push(err.with("foreign_key_list.match")))?,
                    })
                },
            )?;

            let mut invalid_foreign_keys = HashSet::<i64>::new();
            for fk in &mut foreign_key_list {
                if fk.to.is_none() {
                    fk.to = self.get_primary_key(conn, &fk.table, fk.seq, warnings)?;
                    if fk.to.is_none() {
                        invalid_foreign_keys.insert(fk.id);
                    }
                }
            }

            let mut list = ForeignKeyList::new();

            for fk in foreign_key_list {
                if invalid_foreign_keys.contains(&fk.id) {
                    continue;
                }
                list.entry(fk.from).or_default().push(TableSchemaColumnForeignKey {
                    id: fk.id,
                    seq: fk.seq,
                    table: fk.table,
                    to: fk.to.unwrap(),
                    on_update: fk.on_update,
                    on_delete: fk.on_delete,
                    match_: fk.match_,
                });
            }

            self.tables.insert(key.clone(), list);
        }
        Ok(self.tables.get(&key).unwrap())
    }
}

#[derive(Debug)]
struct ForeignKeyListEntry {
    id: i64,
    seq: i64,
    table: String,
    from: String,
    to: Option<String>,
    on_update: String,
    on_delete: String,
    match_: String,
}
