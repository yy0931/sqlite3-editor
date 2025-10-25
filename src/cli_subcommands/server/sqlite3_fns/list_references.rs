use serde::Deserialize;
use serde::Serialize;

use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
use crate::cli_value::CLIValue;
use crate::sqlite_escape::escape_sql_identifier;
use crate::utf8_extractor::get_utf8_string;
use crate::utf8_extractor::InvalidUTF8;

pub fn list_references(
    conn: &rusqlite::Connection,
    table_name: &str,
    column_name: &str,
    value: &CLIValue,
) -> std::result::Result<(Vec<Reference>, Vec<InvalidUTF8>), CLIError> {
    let mut warnings = vec![];

    let primary_key_seq = select_all(
        conn,
        "SELECT pk - 1 FROM pragma_table_info(?) WHERE name = ? AND pk > 0",
        &[table_name.into(), column_name.into()],
        |row| row.get::<_, i64>(0),
    )?
    .first()
    .cloned();

    // foreign keys
    let mut result = select_all(
        conn,
        r#"SELECT t.name, f."from" FROM pragma_table_list t INNER JOIN pragma_foreign_key_list(name) f WHERE t.schema = 'main' COLLATE NOCASE AND NOT (t.name LIKE "sqlite\_%" ESCAPE "\") AND f."table" = ? COLLATE NOCASE AND (f."to" = ? OR (f."to" IS NULL AND f.seq = ?)) COLLATE NOCASE;"#,
        &[table_name.into(), column_name.into(), primary_key_seq.into()],
        |row| {
            Ok(Reference {
                table: get_utf8_string(row, 0, |err| warnings.push(err.with("pragma_table_list.name")))?,
                column: get_utf8_string(row, 1, |err| warnings.push(err.with("pragma_table_list.from")))?,
                count: 0,
            })
        },
    )?;

    for entry in &mut result {
        if let Some(&count) = select_all(
            conn,
            &format!(
                "SELECT COUNT(*) FROM {} WHERE {} IS ?",
                escape_sql_identifier(&entry.table),
                escape_sql_identifier(&entry.column)
            ),
            std::slice::from_ref(value),
            |row| row.get::<_, i64>(0),
        )?
        .first()
        {
            entry.count = count.try_into().unwrap();
        }
    }

    Ok((result, warnings))
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Reference {
    pub table: String,
    pub column: String,
    pub count: u64,
}

#[cfg(test)]
mod test {
    use super::list_references;

    use super::Reference;

    #[test]
    fn test_list_references() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "
CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT;
CREATE TABLE u(y INTEGER NOT NULL REFERENCES t(x));
INSERT INTO t VALUES (1), (2), (3);
INSERT INTO u VALUES (1), (2), (2), (2);",
        )
        .unwrap();
        assert_eq!(
            list_references(&db, "t", "x", &2.into()).unwrap().0,
            vec![Reference {
                table: "u".to_owned(),
                column: "y".to_owned(),
                count: 3,
            }],
        );
    }

    #[test]
    fn test_list_references_shorthand_syntax() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "
CREATE TABLE t(x INTEGER PRIMARY KEY NOT NULL) STRICT;
CREATE TABLE u(y INTEGER NOT NULL REFERENCES t);
INSERT INTO t VALUES (1), (2), (3);
INSERT INTO u VALUES (1), (2), (2), (2);",
        )
        .unwrap();
        assert_eq!(
            list_references(&db, "t", "x", &2.into()).unwrap().0,
            vec![Reference {
                table: "u".to_owned(),
                column: "y".to_owned(),
                count: 3,
            }],
        );
    }

    #[test]
    fn test_list_references_multi_column_foreign_key() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(
            "
CREATE TABLE t(a, b, PRIMARY KEY (b, a));
CREATE TABLE u(x, y, FOREIGN KEY (x, y) REFERENCES t);
INSERT INTO t VALUES (1, 10), (2, 20), (3, 30);
INSERT INTO u VALUES (10, 1), (20, 2), (20, 2);",
        )
        .unwrap();
        assert_eq!(
            list_references(&db, "t", "a", &1.into()).unwrap().0,
            vec![Reference {
                table: "u".to_owned(),
                column: "y".to_owned(),
                count: 1,
            }],
        );
        assert_eq!(
            list_references(&db, "t", "b", &20.into()).unwrap().0,
            vec![Reference {
                table: "u".to_owned(),
                column: "x".to_owned(),
                count: 2,
            }],
        );
    }
}
