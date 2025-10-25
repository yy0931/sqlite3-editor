use crate::cli_error::CLIError;
use crate::cli_value::CLIValue;
use crate::sqlite_escape::escape_sql_identifier;

// Imports a table from a CSV file.
pub fn import(
    database_filepath: &str,
    table_name: &str,
    csv_delimiter: &str,
    input_file: Option<String>,
) -> std::result::Result<(), CLIError> {
    let mut r = csv::ReaderBuilder::new()
        .delimiter(csv_delimiter.as_bytes()[0])
        .from_reader(super::open_reader(input_file)?);

    let columns = r
        .headers()
        .or_else(|err| CLIError::new_other_error(format!("Failed read CSV headers: {err}"), None, None))?
        .into_iter()
        .map(|v| v.to_owned())
        .collect::<Vec<_>>();
    if columns.is_empty() {
        CLIError::new_other_error("No column headers present.", None, None)?;
    }

    let mut con = super::connect(database_filepath)?;
    let tx = con
        .transaction()
        .or_else(|err| CLIError::new_query_error(err, "BEGIN;", &[]))?;
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
        .or_else(|err| CLIError::new_query_error(err, stmt, &[]))?;
    {
        let stmt = format!(
            "INSERT INTO {} VALUES ({})",
            escape_sql_identifier(table_name),
            columns.iter().map(|_| "?").collect::<Vec<_>>().join(", ")
        );
        let mut insert = tx
            .prepare(&stmt)
            .or_else(|err| CLIError::new_query_error(err, &stmt, &[]))?;
        for record in r.records() {
            let values = record?.iter().map(|v| v.to_owned()).collect::<Vec<_>>();
            for (i, value) in values.iter().enumerate() {
                insert.raw_bind_parameter(i + 1, value).or_else(|err| {
                    CLIError::new_query_error(err, &stmt, &values.iter().map(|v| v.into()).collect::<Vec<CLIValue>>())
                })?;
            }
            insert.raw_execute().or_else(|err| {
                CLIError::new_query_error(err, &stmt, &values.iter().map(|v| v.into()).collect::<Vec<CLIValue>>())
            })?;
        }
    }
    tx.commit()
        .or_else(|err| CLIError::new_query_error(err, "COMMIT;", &[]))?;

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::cli_error::CLIError;
    use crate::utf8_extractor::get_utf8_string;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_import_csv() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let mut tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

        // Write a sample CSV file to import.
        writeln!(tmp_csv_file, "name,age\nAlice,20\nBob,25").unwrap();

        // Import the CSV file.
        super::import(tmp_db_filepath, "test", ",", Some(tmp_csv_file_path.to_string())).unwrap();

        // Check the imported data.
        assert_eq!(
            serde_json::to_string(
                &rusqlite::Connection::open(tmp_db_filepath)
                    .unwrap()
                    .prepare("SELECT * FROM test")
                    .unwrap()
                    .query_map([], |row| Ok((
                        get_utf8_string(row, 0, |_| {})?,
                        get_utf8_string(row, 1, |_| {})?
                    )))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            )
            .unwrap(),
            r#"[["Alice","20"],["Bob","25"]]"#
        );
    }

    #[test]
    fn test_import_csv_empty_lines() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let mut tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

        // Write a sample CSV file to import.
        writeln!(tmp_csv_file, "name,age\nAlice,20\nBob,25\n\n").unwrap();

        // Import the CSV file.
        super::import(tmp_db_filepath, "test", ",", Some(tmp_csv_file_path.to_string())).unwrap();

        // Check the imported data.
        assert_eq!(
            serde_json::to_string(
                &rusqlite::Connection::open(tmp_db_filepath)
                    .unwrap()
                    .prepare("SELECT * FROM test")
                    .unwrap()
                    .query_map([], |row| Ok((
                        get_utf8_string(row, 0, |_| {})?,
                        get_utf8_string(row, 1, |_| {})?
                    )))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            )
            .unwrap(),
            r#"[["Alice","20"],["Bob","25"]]"#
        );
    }

    #[test]
    fn test_import_csv_with_inconsistent_columns() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let mut tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

        // Write a sample CSV file to import.
        writeln!(tmp_csv_file, "name,age\nAlice,20\nBob,25,30").unwrap();

        // Import the CSV file.
        assert_eq!(super::import(tmp_db_filepath, "test", ",", Some(tmp_csv_file_path.to_string())), Err(CLIError::Other { message: "CSV error: record 2 (line: 3, byte: 18): found record with 3 fields, but the previous record has 2 fields".to_owned(), query: None, params: None }));
    }

    #[test]
    fn test_import_tsv() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let mut tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

        // Write a sample CSV file to import.
        writeln!(tmp_csv_file, "name\tage\nAlice\t20\nBob\t25").unwrap();

        // Import the CSV file.
        super::import(tmp_db_filepath, "test", "\t", Some(tmp_csv_file_path.to_string())).unwrap();

        // Check the imported data.
        assert_eq!(
            serde_json::to_string(
                &rusqlite::Connection::open(tmp_db_filepath)
                    .unwrap()
                    .prepare("SELECT * FROM test")
                    .unwrap()
                    .query_map([], |row| Ok((
                        get_utf8_string(row, 0, |_| {})?,
                        get_utf8_string(row, 1, |_| {})?
                    )))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            )
            .unwrap(),
            r#"[["Alice","20"],["Bob","25"]]"#
        );
    }

    #[test]
    fn test_empty_csv() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

        // Write a sample CSV file to import.
        fs::write(&tmp_csv_file, r#""#).unwrap();

        // Import the CSV file.
        assert!(super::import(tmp_db_filepath, "test", ",", Some(tmp_csv_file_path))
            .unwrap_err()
            .to_string()
            .contains("No column headers present."));
    }

    #[test]
    fn test_import_csv_header_only() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let mut tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

        // Write a sample CSV file to import.
        writeln!(tmp_csv_file, "name,age").unwrap();

        // Import the CSV file.
        super::import(tmp_db_filepath, "test", ",", Some(tmp_csv_file_path.to_string())).unwrap();
    }
}
