use crate::{
    error::Error,
    sqlite3::{escape_sql_identifier, get_string, set_sqlcipher_key},
    util::into,
};
use base64::{engine::general_purpose, Engine as _};
use once_cell::sync::Lazy;
use rusqlite::{types::ValueRef, Connection};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Write};

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

#[derive(ts_rs::TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(export)]
pub struct CSVExportOptions {
    pub delimiter: String,
    pub null: String,
}

impl Default for CSVExportOptions {
    fn default() -> Self {
        Self {
            delimiter: ",".to_owned(),
            null: "".to_owned(),
        }
    }
}

/// - NULL is encoded as an empty string.
/// - BLOB values are encoded as BASE64 strings.
pub fn export_csv<W: Write>(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    writer: &mut W,
    options: &CSVExportOptions,
) -> std::result::Result<(), Error> {
    // Query
    let con = connect(database_filepath, sql_cipher_key)?;
    let mut stmt = con
        .prepare(query)
        .or_else(|err| Error::new_query_error(err, query, &[]))?;

    if options.delimiter.len() != 1 {
        Error::new_other_error("The delimiter needs to be a single-byte character.", None, None)?;
    }

    // TODO: `stmt.column_count()` and `stmt.column_names()` should be called after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53).
    let column_count = stmt.column_count();
    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut w = csv::WriterBuilder::new()
        .delimiter(options.delimiter.as_bytes()[0])
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
                ValueRef::Null => w.write_field(&options.null)?,
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

    // TODO: `stmt.column_names()` should be called after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53).
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
            serde_json::to_writer::<&mut W, _>(&mut writer, &column_name)?;
            writer.write_all(b":")?;
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => {
                    writer.write_all(b"null")?;
                }
                ValueRef::Real(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &v)?;
                }
                ValueRef::Blob(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &general_purpose::STANDARD.encode(v))?;
                }
                ValueRef::Integer(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &v)?;
                }
                ValueRef::Text(v) => {
                    serde_json::to_writer::<&mut W, _>(&mut writer, &String::from_utf8_lossy(v))?;
                }
            }
        }
        writer.write_all(b"}")?;
    }
    writer.write_all(b"]")?;

    Ok(())
}

#[derive(ts_rs::TS, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[ts(export)]
pub struct XLSXExportOptions {
    pub hidden_columns: Option<Vec<u16>>,
    pub selection: Option<(u32, u16, u32, u16)>,
    pub top_left: Option<(u32, u16)>,
    pub wrap_text: bool,
    pub all_tables: bool,
    pub active_sheet: Option<String>,
}

static INVALID_SHEET_NAME_PATTERN: Lazy<regex::Regex> = Lazy::new(|| regex::Regex::new(r"[*?:\[\]\\/]|^'|'$").unwrap());

/// - Integer values between i32::MIN and i32::MAX are encoded as i32. Values outside this range are rounded to the nearest f64 values because Excel does not support 64-bit integers.
/// - NULL is encoded as an empty string.
/// - BLOB values are encoded as BASE64 strings.
pub fn export_xlsx(
    database_filepath: &str,
    sql_cipher_key: &Option<String>,
    query: &str,
    output_file: &str,
    options: &XLSXExportOptions,
) -> std::result::Result<(), Error> {
    // Query
    let mut con = connect(database_filepath, sql_cipher_key)?;

    let mut workbook = rust_xlsxwriter::Workbook::new();

    if options.all_tables {
        // List tables
        // NOTE: Views are excluded because they can return an infinite number of records.
        let pragma_table_list = r#"SELECT name FROM pragma_table_list WHERE NOT (name LIKE "sqlite\_%" ESCAPE "\") AND schema = 'main' AND type = 'table' COLLATE NOCASE"#;
        let mut stmt = con
            .prepare(pragma_table_list)
            .or_else(|err| Error::new_query_error(err, pragma_table_list, &[]))?;
        let table_names = stmt
            .query_map([], |row| get_string(row, 0, |_| { /* ignore utf-8 decoding errors */ }))
            .or_else(|err| Error::new_query_error(err, pragma_table_list, &[]))?
            .collect::<Result<Vec<_>, _>>()
            .or_else(|err| Error::new_query_error(err, pragma_table_list, &[]))?;
        drop(stmt);

        // Fix table names
        let mut sheet_names = HashSet::<String>::new();

        // Add worksheets
        for table_name in table_names {
            let worksheet = workbook.add_worksheet();

            let mut sheet_name = INVALID_SHEET_NAME_PATTERN.replace_all(&table_name, "").to_string();
            if sheet_name.is_empty() {
                sheet_name = "\"\"".to_owned();
            };

            // Trim the sheet name and add a suffix if the sheet name is already taken
            let mut sheet_name_with_suffix = sheet_name.chars().take(31).collect();
            let mut suffix_count = 1;
            while sheet_names.contains(&sheet_name_with_suffix) {
                suffix_count += 1;
                sheet_name_with_suffix = format!(
                    "{}_{suffix_count}",
                    sheet_name
                        .chars()
                        .take(31 - 1 - suffix_count.to_string().len())
                        .collect::<String>()
                );
            }

            let _ = worksheet.set_name(&sheet_name_with_suffix);
            sheet_names.insert(sheet_name_with_suffix);

            write_table_data(
                &mut con,
                &format!("SELECT * FROM {}", escape_sql_identifier(&table_name)),
                worksheet,
                options.wrap_text,
            )?;

            if options.active_sheet == Some(table_name) {
                worksheet.set_active(true);
            }
        }
    } else {
        let worksheet = workbook.add_worksheet();
        write_table_data(&mut con, query, worksheet, options.wrap_text)?;
        if let Some(hidden_columns) = &options.hidden_columns {
            for col_id in hidden_columns {
                worksheet.set_column_hidden(*col_id)?;
            }
        }
        if let Some((first_row, first_col, last_row, last_col)) = options.selection {
            worksheet.set_selection(first_row + 1, first_col, last_row + 1, last_col)?;
        }
        if let Some((row, col)) = options.top_left {
            worksheet.set_top_left_cell(if row == 0 { 0 } else { row + 1 }, col)?;
        }
    }

    workbook.save(output_file)?;

    Ok(())
}

fn write_table_data(
    con: &mut Connection,
    query: &str,
    worksheet: &mut rust_xlsxwriter::Worksheet,
    wrap_text: bool,
) -> std::result::Result<(), Error> {
    let mut stmt = con
        .prepare(query)
        .or_else(|err| Error::new_query_error(err, query, &[]))?;

    // TODO: `stmt.column_count()` and `stmt.column_names()` should be called after `rows.next()` (see https://github.com/rusqlite/rusqlite/blob/b7309f2dca70716fee44c85082c585b330edb073/src/column.rs#L51-L53).
    let column_count = stmt.column_count();
    let column_names = stmt
        .column_names()
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();

    let mut row_id = 0u32;
    for (col_id, name) in column_names.iter().enumerate() {
        worksheet.write(row_id, col_id.try_into().unwrap(), name)?;
        worksheet.set_column_width(col_id as u16, 13)?;
        if wrap_text {
            worksheet.set_column_format(col_id as u16, &rust_xlsxwriter::Format::new().set_text_wrap())?;
        }
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

    if row_id > 1 {
        let table = rust_xlsxwriter::Table::new();
        worksheet.add_table(0, 0, row_id - 1, (column_count - 1) as u16, &table)?;
    }

    Ok(())
}
