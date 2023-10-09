use crate::{
    error::Error,
    literal::Literal,
    sqlite3_driver::{escape_sql_identifier, set_sqlcipher_key},
    FileFormat,
};
use std::collections::HashMap;
use std::io::Read;

pub fn import(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    format: FileFormat,
    table_name: &str,
    mut csv_delimiter: &str,
    input_file: Option<String>,
) -> std::result::Result<(), Error> {
    // Connect to the database
    let mut con = rusqlite::Connection::open(database_filepath).or_else(|err| {
        Error::new_other_error(
            format!("Failed to open the database {database_filepath:?}: {err}"),
            None,
            None,
        )
    })?;

    // Set the SQLite Cipher key if given
    if let Some(key) = sql_cipher_key {
        set_sqlcipher_key(&con, key.as_ref())?;
    }

    let input: Box<dyn Read> = if let Some(input_file) = input_file {
        Box::new(std::fs::File::open(&input_file).or_else(|err| {
            Error::new_other_error(format!("Failed to open the database {input_file:?}: {err}"), None, None)
        })?)
    } else {
        // expected `File`, found `Stdin`
        Box::new(std::io::stdin())
    };

    match format {
        FileFormat::CSV | FileFormat::TSV => {
            if format == FileFormat::TSV {
                csv_delimiter = "\t";
            }

            let mut r = csv::ReaderBuilder::new()
                .delimiter(csv_delimiter.as_bytes()[0])
                .from_reader(input);

            let columns = r
                .headers()
                .or_else(|err| Error::new_other_error(format!("Failed read CSV headers: {err}"), None, None))?
                .into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<_>>();
            if columns.is_empty() {
                Error::new_other_error("No column headers present.", None, None)?;
            }

            let tx = con
                .transaction()
                .or_else(|err| Error::new_query_error(err, "BEGIN;", &[]))?;
            let stmt = format!(
                "CREATE TABLE {}({})",
                escape_sql_identifier(table_name),
                columns
                    .iter()
                    .map(|v| format!("{} TEXT", escape_sql_identifier(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            tx.execute(&stmt, [])
                .or_else(|err| Error::new_query_error(err, stmt, &[]))?;
            {
                let stmt = format!(
                    "INSERT INTO {} VALUES ({})",
                    escape_sql_identifier(table_name),
                    columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
                );
                let mut insert = tx
                    .prepare(&stmt)
                    .or_else(|err| Error::new_query_error(err, &stmt, &[]))?;
                for record in r.records() {
                    let values = record?.iter().map(|v| v.to_owned()).collect::<Vec<_>>();
                    for (i, value) in values.iter().enumerate() {
                        insert.raw_bind_parameter(i + 1, value).or_else(|err| {
                            Error::new_query_error(
                                err,
                                &stmt,
                                &values.iter().map(|v| v.into()).collect::<Vec<Literal>>(),
                            )
                        })?;
                    }
                    insert.raw_execute().or_else(|err| {
                        Error::new_query_error(err, &stmt, &values.iter().map(|v| v.into()).collect::<Vec<Literal>>())
                    })?;
                }
            }
            tx.commit().or_else(|err| Error::new_query_error(err, "COMMIT;", &[]))?;
        }
        FileFormat::JSON => {
            use serde_json::Value;

            // This can fail if `input` is not proper JSON.
            // NOTE: serde_Json does not maintain the key order and using BTreeMap instead of HashMap didn't help. https://stackoverflow.com/a/75954625/10710682 might help.
            let parsed: Vec<HashMap<String, String>> =
                serde_json::from_reader::<_, Vec<HashMap<String, Value>>>(input)?
                    .into_iter()
                    .map(|map| {
                        map.into_iter()
                            .map(|(k, v)| {
                                (
                                    k,
                                    match v {
                                        Value::String(s) => s,
                                        v => v.to_string(),
                                    },
                                )
                            })
                            .collect()
                    })
                    .collect();

            if parsed.is_empty() {
                Error::new_other_error("No data present.", None, None)?;
            }

            let columns = parsed
                .first()
                .unwrap()
                .keys()
                .into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<_>>();

            if columns.is_empty() {
                Error::new_other_error("No column headers present.", None, None)?;
            }

            let tx = con
                .transaction()
                .or_else(|err| Error::new_query_error(err, "BEGIN;", &[]))?;
            let stmt = format!(
                "CREATE TABLE {}({})",
                escape_sql_identifier(&table_name),
                columns
                    .iter()
                    .map(|v| format!("{} TEXT", escape_sql_identifier(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            tx.execute(&stmt, [])
                .or_else(|err| Error::new_query_error(err, stmt, &[]))?;
            {
                let stmt = format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    escape_sql_identifier(&table_name),
                    columns
                        .iter()
                        .map(|v| escape_sql_identifier(v))
                        .collect::<Vec<_>>()
                        .join(", "),
                    columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
                );
                let mut insert = tx
                    .prepare(&stmt)
                    .or_else(|err| Error::new_query_error(err, &stmt, &[]))?;
                for record in &parsed {
                    let values = columns.iter().map(|column| record.get(column)).collect::<Vec<_>>();
                    for (i, value) in values.iter().enumerate() {
                        insert.raw_bind_parameter(i + 1, value).or_else(|err| {
                            Error::new_query_error(
                                err,
                                &stmt,
                                &values.iter().map(|v| (*v).into()).collect::<Vec<Literal>>(),
                            )
                        })?;
                    }
                    insert.raw_execute().or_else(|err| {
                        Error::new_query_error(
                            err,
                            &stmt,
                            &values.iter().map(|v| (*v).into()).collect::<Vec<Literal>>(),
                        )
                    })?;
                }
            }
            tx.commit().or_else(|err| Error::new_query_error(err, "COMMIT;", &[]))?;
        }
    }

    Ok(())
}
