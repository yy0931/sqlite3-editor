use anyhow::bail;
use std::collections::HashMap;
use std::io::Read;

use crate::{
    sqlite3_driver::{escape_sql_identifier, set_sqlcipher_key},
    FileFormat,
};

pub fn import(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    format: FileFormat,
    table_name: &str,
    mut csv_delimiter: &str,
    input_file: Option<String>,
) -> anyhow::Result<()> {
    // Connect to the database
    let mut con = rusqlite::Connection::open(database_filepath)
        .or_else(|err| bail!("Failed to open the database {database_filepath:?}: {err}"))?;

    // Set the SQLite Cipher key if given
    if let Some(key) = sql_cipher_key {
        set_sqlcipher_key(&con, key.as_ref()).or_else(|err| bail!(err))?;
    }

    let input: Box<dyn Read> = if let Some(input_file) = input_file {
        Box::new(std::fs::File::open(input_file)?)
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

            let columns = r.headers()?.into_iter().map(|v| v.to_owned()).collect::<Vec<_>>();
            if columns.is_empty() {
                eprintln!("No column headers present.");
                std::process::exit(1);
            }

            let tx = con.transaction()?;
            tx.execute(
                &format!(
                    "CREATE TABLE {}({})",
                    escape_sql_identifier(table_name),
                    columns
                        .iter()
                        .map(|v| format!("{} TEXT", escape_sql_identifier(v)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                [],
            )?;
            {
                let mut insert = tx.prepare(&format!(
                    "INSERT INTO {} VALUES ({})",
                    escape_sql_identifier(table_name),
                    columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
                ))?;
                for record in r.records() {
                    for (i, value) in record?.iter().enumerate() {
                        insert.raw_bind_parameter(i + 1, value)?;
                    }
                    insert.raw_execute()?;
                }
            }
            tx.commit()?;
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
                eprintln!("No data present.");
                std::process::exit(1);
            }

            let columns = parsed
                .first()
                .unwrap()
                .keys()
                .into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<_>>();

            if columns.is_empty() {
                eprintln!("No column headers present.");
                std::process::exit(1);
            }

            let tx = con.transaction()?;
            tx.execute(
                &format!(
                    "CREATE TABLE {}({})",
                    escape_sql_identifier(&table_name),
                    columns
                        .iter()
                        .map(|v| format!("{} TEXT", escape_sql_identifier(v)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                [],
            )?;
            {
                let mut insert = tx.prepare(&format!(
                    "INSERT INTO {} ({}) VALUES ({})",
                    escape_sql_identifier(&table_name),
                    columns
                        .iter()
                        .map(|v| escape_sql_identifier(v))
                        .collect::<Vec<_>>()
                        .join(", "),
                    columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
                ))?;
                for record in &parsed {
                    for (i, column) in columns.iter().enumerate() {
                        insert.raw_bind_parameter(i + 1, record.get(column))?;
                    }
                    insert.raw_execute()?;
                }
            }
            tx.commit()?;
        }
    }

    Ok(())
}
