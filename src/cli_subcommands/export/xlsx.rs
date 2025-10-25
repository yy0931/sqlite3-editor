use crate::cli_error::CLIError;
use crate::sqlite_escape::escape_sql_identifier;
use crate::utf8_extractor::get_utf8_string;
use base64::engine::general_purpose;
use base64::Engine as _;
use once_cell::sync::Lazy;
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;

/// Exports a table or a database as a xlsx file.
/// - Integer values between i32::MIN and i32::MAX are encoded as i32. Values outside this range are rounded to the nearest f64 values because Excel does not support 64-bit integers.
/// - NULL is encoded as an empty string.
/// - BLOB values are encoded as BASE64 strings.
pub fn export(
    database_filepath: &str,
    query: &str,
    output_file: &str,
    options: &XLSXExportOptions,
) -> std::result::Result<(), CLIError> {
    // Query
    let mut con = super::connect(database_filepath)?;

    let mut workbook = rust_xlsxwriter::Workbook::new();

    if options.all_tables {
        // List tables
        // NOTE: Views are excluded because they can return an infinite number of records.
        let pragma_table_list = r#"SELECT name FROM pragma_table_list WHERE NOT (name LIKE "sqlite\_%" ESCAPE "\") AND schema = 'main' AND type = 'table' COLLATE NOCASE"#;
        let mut stmt = con
            .prepare(pragma_table_list)
            .or_else(|err| CLIError::new_query_error(err, pragma_table_list, &[]))?;
        let table_names = stmt
            .query_map([], |row| {
                get_utf8_string(row, 0, |_| { /* ignore utf-8 decoding errors */ })
            })
            .or_else(|err| CLIError::new_query_error(err, pragma_table_list, &[]))?
            .collect::<Result<Vec<_>, _>>()
            .or_else(|err| CLIError::new_query_error(err, pragma_table_list, &[]))?;
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

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize, ts_rs::TS)]
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

fn write_table_data(
    con: &mut Connection,
    query: &str,
    worksheet: &mut rust_xlsxwriter::Worksheet,
    wrap_text: bool,
) -> std::result::Result<(), CLIError> {
    let mut stmt = con
        .prepare(query)
        .or_else(|err| CLIError::new_query_error(err, query, &[]))?;

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

    let mut rows = stmt
        .query([])
        .or_else(|err| CLIError::new_query_error(err, query, &[]))?;
    while let Some(row) = rows.next().or_else(|err| CLIError::new_query_error(err, query, &[]))? {
        for col_id in 0..column_count {
            match row.get_ref_unwrap(col_id) {
                ValueRef::Null => worksheet.write(row_id, col_id.try_into().unwrap(), "")?,
                ValueRef::Real(v) => worksheet.write(row_id, col_id.try_into().unwrap(), v)?,
                ValueRef::Blob(v) => {
                    worksheet.write(row_id, col_id.try_into().unwrap(), general_purpose::STANDARD.encode(v))?
                }
                ValueRef::Integer(v) if i32::MIN as i64 <= v && v <= i32::MAX as i64 => {
                    worksheet.write(row_id, col_id.try_into().unwrap(), TryInto::<i32>::try_into(v).unwrap())?
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

#[cfg(test)]
mod test {
    use super::XLSXExportOptions;

    #[test]
    fn test_export_xlsx() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

        let tmp_out = tempfile::NamedTempFile::new().unwrap();
        let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

        super::super::setup_test_db(&tmp_db_filepath);

        super::export(
            &tmp_db_filepath,
            "SELECT * FROM test",
            &tmp_out_path,
            &XLSXExportOptions::default(),
        )
        .unwrap();

        dbg!(tmp_out.as_file().metadata().unwrap().len());
    }
}
