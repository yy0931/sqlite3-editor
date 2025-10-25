use crate::cli_error::CLIError;
use crate::ExportingFileFormat;
mod csv;
mod json;
mod xlsx;

use std::io::Write;

// Exports a table or database to a CSV, JSON, or XLSX file.
pub fn run(
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    database_filepath: String,
    format: ExportingFileFormat,
    query: String,
    output_file: Option<String>,
    csv_options: Option<String>,
    xlsx_options: Option<String>,
) -> i32 {
    if format == ExportingFileFormat::Xlsx {
        let Some(output_file) = output_file else {
            writeln!(stderr, "`--format xlsx` requires `--output-file <file-name>`.").expect("writeln! failed.");
            return 1;
        };
        let options: xlsx::XLSXExportOptions = if let Some(s) = xlsx_options {
            serde_json::from_str(&s).unwrap_or_default()
        } else {
            xlsx::XLSXExportOptions::default()
        };
        if let Err(err) = self::xlsx::export(&database_filepath, &query, &output_file, &options) {
            writeln!(stderr, "{err}").expect("writeln! failed.");
            return 1;
        }
    } else {
        let mut writer: Box<dyn Write> = if let Some(output_file) = output_file {
            let Ok(f) = std::fs::OpenOptions::new()
                .truncate(true)
                .create(true)
                .write(true)
                .open(output_file)
            else {
                return 1;
            };
            Box::new(f)
        } else {
            Box::new(stdout)
        };

        if let Err(err) = match format {
            ExportingFileFormat::Csv => {
                let options: csv::CSVExportOptions = if let Some(s) = csv_options {
                    serde_json::from_str(&s).unwrap_or_default()
                } else {
                    csv::CSVExportOptions::default()
                };
                self::csv::export(&database_filepath, &query, &mut writer, &options)
            }
            ExportingFileFormat::Json => self::json::export(&database_filepath, &query, &mut writer),
            ExportingFileFormat::Xlsx => {
                panic!();
            }
        } {
            writeln!(stderr, "{err}").expect("writeln! failed.");
            return 1;
        }
    }

    0
}

fn connect(database_filepath: &str) -> std::result::Result<rusqlite::Connection, CLIError> {
    // Connect to the database
    let con = rusqlite::Connection::open(database_filepath).or_else(|err| {
        CLIError::new_other_error(
            format!("Failed to open the database {database_filepath:?}: {err}"),
            None,
            None,
        )
    })?;

    Ok(con)
}

#[cfg(test)]
fn setup_test_db(tmp_db_filepath: &str) {
    let connection = rusqlite::Connection::open(tmp_db_filepath).unwrap();

    connection
        .execute(
            "CREATE TABLE test (t TEXT NOT NULL, i INTEGER NOT NULL, n ANY, r REAL, b BLOB);",
            [],
        )
        .unwrap();

    connection
        .execute(
            "INSERT INTO test VALUES (?, ?, ?, ?, ?)",
            ("Alice", 20, None::<String>, 1.2, vec![1, 2, 3]),
        )
        .unwrap();

    connection
        .execute(
            "INSERT INTO test VALUES (?, ?, ?, ?, ?)",
            ("Alice", 25, None::<String>, 2.4, vec![4, 5, 6]),
        )
        .unwrap();
}
