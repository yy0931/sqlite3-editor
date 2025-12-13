mod csv;
mod json;

use crate::cli_error::CLIError;
use crate::ImportingFileFormat;
use std::io::Read;
use std::io::Write;

// Imports a table from a CSV or JSON file.
pub fn run(
    stderr: &mut impl Write,
    database_filepath: String,
    format: ImportingFileFormat,
    table_name: String,
    csv_delimiter: String,
    input_file: Option<String>,
) -> i32 {
    if let Err(err) = match format {
        ImportingFileFormat::Csv => self::csv::import(&database_filepath, &table_name, &csv_delimiter, input_file),
        ImportingFileFormat::Tsv => self::csv::import(&database_filepath, &table_name, "\t", input_file),
        ImportingFileFormat::Json => self::json::import(&database_filepath, &table_name, input_file),
    } {
        writeln!(stderr, "{err}").expect("writeln! failed.");
        return 1;
    }

    0
}

fn connect(database_filepath: &str) -> std::result::Result<rusqlite::Connection, CLIError> {
    // Connect to the database
    let con = rusqlite::Connection::open(database_filepath)
        .or_else(|err| CLIError::new_other_error(format!("Failed to open the database {database_filepath:?}: {err}"), None, None))?;

    Ok(con)
}

fn open_reader(input_file: Option<String>) -> std::result::Result<Box<dyn Read>, CLIError> {
    Ok(if let Some(input_file) = input_file {
        Box::new(
            std::fs::File::open(&input_file)
                .or_else(|err| CLIError::new_other_error(format!("Failed to open the database {input_file:?}: {err}"), None, None))?,
        )
    } else {
        // expected `File`, found `Stdin`
        Box::new(std::io::stdin())
    })
}
