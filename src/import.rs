use crate::{
    error::Error,
    literal::Literal,
    sqlite3_driver::{escape_sql_identifier, set_sqlcipher_key},
};
use std::collections::HashMap;
use std::io::Read;

fn connect(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
) -> std::result::Result<rusqlite::Connection, Error> {
    // Connect to the database
    let con = rusqlite::Connection::open(database_filepath).or_else(|err| {
        Error::new_other_error(
            format!("Failed to open the database {database_filepath:?}: {err}"),
            None,
            None,
        )
    })?;

    // Set the SQLite Cipher key if given
    if let Some(key) = sql_cipher_key {
        set_sqlcipher_key(&con, key)?;
    }

    Ok(con)
}

fn open_reader(input_file: Option<String>) -> std::result::Result<Box<dyn Read>, Error> {
    Ok(if let Some(input_file) = input_file {
        Box::new(std::fs::File::open(&input_file).or_else(|err| {
            Error::new_other_error(format!("Failed to open the database {input_file:?}: {err}"), None, None)
        })?)
    } else {
        // expected `File`, found `Stdin`
        Box::new(std::io::stdin())
    })
}

pub fn import_csv(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    table_name: &str,
    csv_delimiter: &str,
    input_file: Option<String>,
) -> std::result::Result<(), Error> {
    let mut r = csv::ReaderBuilder::new()
        .delimiter(csv_delimiter.as_bytes()[0])
        .from_reader(open_reader(input_file)?);

    let columns = r
        .headers()
        .or_else(|err| Error::new_other_error(format!("Failed read CSV headers: {err}"), None, None))?
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        Error::new_other_error("No column headers present.", None, None)?;
    }

    let mut con = connect(database_filepath, sql_cipher_key)?;
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
                    Error::new_query_error(err, &stmt, &values.iter().map(|v| v.into()).collect::<Vec<Literal>>())
                })?;
            }
            insert.raw_execute().or_else(|err| {
                Error::new_query_error(err, &stmt, &values.iter().map(|v| v.into()).collect::<Vec<Literal>>())
            })?;
        }
    }
    tx.commit().or_else(|err| Error::new_query_error(err, "COMMIT;", &[]))?;

    Ok(())
}

pub fn import_json(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    table_name: &str,
    input_file: Option<String>,
) -> std::result::Result<(), Error> {
    use serde_json::Value;

    let parsed: Vec<HashMap<String, String>> =
        serde_json::from_reader::<_, Vec<HashMap<String, Value>>>(open_reader(input_file)?)?
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

    let columns = parsed.first().unwrap().keys().map(|v| v.to_owned()).collect::<Vec<_>>();

    if columns.is_empty() {
        Error::new_other_error("No column headers present.", None, None)?;
    }

    let mut con = connect(database_filepath, sql_cipher_key)?;
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
            "INSERT INTO {} ({}) VALUES ({})",
            escape_sql_identifier(table_name),
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

    Ok(())
}
