use crate::{error::Error, sqlite3_driver::set_sqlcipher_key, FileFormat};
use rusqlite::types::ValueRef;
use std::io::Write;

pub fn export(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    format: FileFormat,
    mut csv_delimiter: &str,
    output_file: Option<String>,
) -> std::result::Result<(), Error> {
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

    // Query
    let mut stmt = con
        .prepare(query)
        .or_else(|err| Error::new_query_error(err, query, &[]))?;
    if csv_delimiter.as_bytes().len() != 1 {
        Error::new_other_error("csv_delimiter needs to be a single character.", None, None)?;
    }

    let column_count = stmt.column_count();
    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut out: Box<dyn Write> = if let Some(output_file) = output_file {
        Box::new(
            std::fs::OpenOptions::new()
                .truncate(true)
                .create(true)
                .write(true)
                .open(output_file)?,
        )
    } else {
        Box::new(std::io::stdout())
    };

    use base64::{engine::general_purpose, Engine as _};

    match format {
        FileFormat::CSV | FileFormat::TSV => {
            if format == FileFormat::TSV {
                csv_delimiter = "\t";
            }

            let mut w = csv::WriterBuilder::new()
                .delimiter(csv_delimiter.as_bytes()[0])
                .from_writer(out);

            // Header
            for name in column_names {
                w.write_field(name)?;
            }
            w.write_record(None::<&[u8]>)?;

            let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
            while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
                for i in 0..column_count {
                    match row.get_ref_unwrap(i) {
                        ValueRef::Null => w.write_field("")?,
                        ValueRef::Real(v) => w.write_field(format!("{v}"))?,
                        ValueRef::Blob(v) => w.write_field(general_purpose::STANDARD.encode(v))?,
                        ValueRef::Integer(v) => w.write_field(format!("{v}"))?,
                        ValueRef::Text(v) => w.write_field(v)?,
                    };
                }
                w.write_record(None::<&[u8]>)?;
            }
        }
        FileFormat::JSON => {
            out.write(b"[")?;
            let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
            let mut first_entry = true;
            while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
                if !first_entry {
                    out.write(b",")?;
                }
                first_entry = false;
                out.write(b"{")?;
                for i in 0..column_count {
                    if i != 0 {
                        out.write(b",")?;
                    }
                    serde_json::to_writer(&mut out, &column_names[i])?;
                    out.write(b":")?;
                    match row.get_ref_unwrap(i) {
                        ValueRef::Null => {
                            out.write(b"null")?;
                        }
                        ValueRef::Real(v) => {
                            serde_json::to_writer(&mut out, &v)?;
                        }
                        ValueRef::Blob(v) => {
                            serde_json::to_writer(&mut out, &general_purpose::STANDARD.encode(v))?;
                        }
                        ValueRef::Integer(v) => {
                            serde_json::to_writer(&mut out, &v)?;
                        }
                        ValueRef::Text(v) => {
                            serde_json::to_writer(&mut out, &String::from_utf8_lossy(v))?;
                        }
                    }
                }
                out.write(b"}")?;
            }
            out.write(b"]")?;
        }
    }

    Ok(())
}
