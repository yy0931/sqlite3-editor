use crate::{error::Error, sqlite3_driver::set_sqlcipher_key, util::into};
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

/// - NULL is encoded as an empty string.
/// - BLOB values are encoded as BASE64 strings.
pub fn export_csv<W: Write>(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    csv_delimiter: &str,
    writer: &mut W,
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
        .from_writer(writer);

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

/// - BLOB values are encoded as BASE64 strings.
pub fn export_json<W: Write>(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    mut writer: &mut W,
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

    writer.write_all(b"[")?;
    let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
    let mut first_entry = true;
    while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
        if !first_entry {
            writer.write_all(b",")?;
        }
        first_entry = false;
        writer.write_all(b"{")?;
        for (col_id, column_name) in column_names.iter().enumerate() {
            if col_id != 0 {
                writer.write_all(b",")?;
            }
            serde_json::to_writer(&mut writer, &column_name)?;
            writer.write_all(b":")?;
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => {
                    writer.write_all(b"null")?;
                }
                ValueRef::Real(v) => {
                    serde_json::to_writer(&mut writer, &v)?;
                }
                ValueRef::Blob(v) => {
                    serde_json::to_writer(&mut writer, &general_purpose::STANDARD.encode(v))?;
                }
                ValueRef::Integer(v) => {
                    serde_json::to_writer(&mut writer, &v)?;
                }
                ValueRef::Text(v) => {
                    serde_json::to_writer(&mut writer, &String::from_utf8_lossy(v))?;
                }
            }
        }
        writer.write_all(b"}")?;
    }
    writer.write_all(b"]")?;

    Ok(())
}

/// - Integer values between i32::MIN and i32::MAX are encoded as i32. Values outside this range are rounded to the nearest f64 values because Excel does not support 64-bit integers.
/// - NULL is encoded as an empty string.
/// - BLOB values are encoded as BASE64 strings.
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
        worksheet.write(row_id, col_id.try_into().unwrap(), name)?;
    }
    row_id += 1;

    let mut rows = stmt.query([]).or_else(|err| Error::new_query_error(err, query, &[]))?;
    while let Some(row) = rows.next().or_else(|err| Error::new_query_error(err, query, &[]))? {
        for col_id in 0..column_count {
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => worksheet.write(row_id, col_id.try_into().unwrap(), "")?,
                ValueRef::Real(v) => worksheet.write(row_id, col_id.try_into().unwrap(), v)?,
                ValueRef::Blob(v) => {
                    worksheet.write(row_id, col_id.try_into().unwrap(), general_purpose::STANDARD.encode(v))?
                }
                ValueRef::Integer(v) if i32::MIN as i64 <= v && v <= i32::MAX as i64 => {
                    worksheet.write(row_id, col_id.try_into().unwrap(), into::<_, i32>(v))?
                }
                ValueRef::Integer(v) => worksheet.write(row_id, col_id.try_into().unwrap(), v as f64)?,
                ValueRef::Text(v) => worksheet.write(row_id, col_id.try_into().unwrap(), String::from_utf8_lossy(v))?,
            };
        }
        row_id += 1;
    }
    workbook.save(output_file)?;

    Ok(())
}
