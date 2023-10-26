use crate::{error::Error, sqlite3_driver::set_sqlcipher_key};
use base64::{engine::general_purpose, Engine as _};
use rusqlite::types::ValueRef;
use std::io::Write;

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

fn open_writer(output_file: Option<String>) -> std::io::Result<Box<dyn Write>> {
    Ok(if let Some(output_file) = output_file {
        Box::new(
            std::fs::OpenOptions::new()
                .truncate(true)
                .create(true)
                .write(true)
                .open(output_file)?,
        )
    } else {
        Box::new(std::io::stdout())
    })
}

pub fn export_csv(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    csv_delimiter: &str,
    output_file: Option<String>,
) -> std::result::Result<(), Error> {
    // Query
    let con = connect(database_filepath, sql_cipher_key)?;
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

    let mut w = csv::WriterBuilder::new()
        .delimiter(csv_delimiter.as_bytes()[0])
        .from_writer(open_writer(output_file)?);

    // Header
    for name in column_names {
        w.write_field(name)?;
    }
    w.write_record(None::<&[u8]>)?;

    let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
    while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
        for col_id in 0..column_count {
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => w.write_field("")?,
                ValueRef::Real(v) => w.write_field(format!("{v}"))?,
                ValueRef::Blob(v) => w.write_field(general_purpose::STANDARD.encode(v))?,
                ValueRef::Integer(v) => w.write_field(format!("{v}"))?,
                ValueRef::Text(v) => w.write_field(v)?,
            };
        }
        w.write_record(None::<&[u8]>)?;
    }

    Ok(())
}

pub fn export_json(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    output_file: Option<String>,
) -> std::result::Result<(), Error> {
    // Query
    let con = connect(database_filepath, sql_cipher_key)?;
    let mut stmt = con
        .prepare(query)
        .or_else(|err| Error::new_query_error(err, query, &[]))?;

    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut out = open_writer(output_file)?;
    out.write_all(b"[")?;
    let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
    let mut first_entry = true;
    while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
        if !first_entry {
            out.write_all(b",")?;
        }
        first_entry = false;
        out.write_all(b"{")?;
        for (col_id, column_name) in column_names.iter().enumerate() {
            if col_id != 0 {
                out.write_all(b",")?;
            }
            serde_json::to_writer(&mut out, &column_name)?;
            out.write_all(b":")?;
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => {
                    out.write_all(b"null")?;
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
        out.write_all(b"}")?;
    }
    out.write_all(b"]")?;

    Ok(())
}

pub fn export_xlsx(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    output_file: &str,
) -> std::result::Result<(), Error> {
    // Query
    let con = connect(database_filepath, sql_cipher_key)?;
    let mut stmt = con
        .prepare(query)
        .or_else(|err| Error::new_query_error(err, query, &[]))?;

    let column_count = stmt.column_count();
    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut workbook = rust_xlsxwriter::Workbook::new();
    let worksheet = workbook.add_worksheet();

    let mut row_id = 0u32;
    for (col_id, name) in column_names.iter().enumerate() {
        worksheet.write(row_id, col_id as u16, name)?;
        row_id += 1;
    }

    let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
    while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
        for col_id in 0..column_count {
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => worksheet.write(row_id, col_id as u16, "")?,
                ValueRef::Real(v) => worksheet.write(row_id, col_id as u16, v)?,
                ValueRef::Blob(v) => worksheet.write(row_id, col_id as u16, general_purpose::STANDARD.encode(v))?,
                ValueRef::Integer(v) if i32::MIN as i64 <= v && v <= i32::MAX as i64 => {
                    worksheet.write(row_id, col_id as u16, v as i32)?
                }
                ValueRef::Integer(v) => worksheet.write(row_id, col_id as u16, format!("{v}"))?,
                ValueRef::Text(v) => worksheet.write(row_id, col_id as u16, String::from_utf8_lossy(v))?,
            };
            row_id += 1;
        }
    }
    workbook.save(output_file)?;

    Ok(())
}
