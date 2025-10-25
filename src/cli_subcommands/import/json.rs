use crate::cli_error::CLIError;
use crate::cli_value::CLIValue;
use crate::sqlite_escape::escape_sql_identifier;
use std::collections::HashMap;

// Imports a table from a JSON file.
pub fn import(
    database_filepath: &str,
    table_name: &str,
    input_file: Option<String>,
) -> std::result::Result<(), CLIError> {
    let parsed = serde_json::from_reader::<_, Vec<HashMap<String, CLIValue>>>(super::open_reader(input_file)?)?;

    if parsed.is_empty() {
        return CLIError::new_other_error("No data present.", None, None);
    }

    let columns = parsed.first().unwrap().keys().map(|v| v.to_owned()).collect::<Vec<_>>();

    if columns.is_empty() {
        return CLIError::new_other_error("No column headers present.", None, None);
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
            .map(|v| escape_sql_identifier(v))
            .collect::<Vec<_>>()
            .join(", ")
    );
    tx.execute(&stmt, [])
        .or_else(|err| CLIError::new_query_error(err, stmt, &[]))?;
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
            .or_else(|err| CLIError::new_query_error(err, &stmt, &[]))?;
        for (record_id, record) in parsed.iter().enumerate() {
            let mut values = Vec::<&CLIValue>::new();
            for column in &columns {
                let Some(value) = record.get(column) else {
                    return CLIError::new_other_error(
                        format!("The row {} does not have the column '{column}'.", record_id + 1),
                        None,
                        None,
                    );
                };
                values.push(value);
            }
            for (i, value) in values.iter().enumerate() {
                insert.raw_bind_parameter(i + 1, value).or_else(|err| {
                    CLIError::new_query_error(
                        err,
                        &stmt,
                        &values.iter().map(|&v| v.clone()).collect::<Vec<CLIValue>>(),
                    )
                })?;
            }
            insert.raw_execute().or_else(|err| {
                CLIError::new_query_error(
                    err,
                    &stmt,
                    &values.iter().map(|&v| v.clone()).collect::<Vec<CLIValue>>(),
                )
            })?;
        }
    }
    tx.commit()
        .or_else(|err| CLIError::new_query_error(err, "COMMIT;", &[]))?;

    Ok(())
}

#[cfg(test)]
mod test {
    use std::fs;

    #[test]
    fn test_import_json() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let tmp_json_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_json_file_path = tmp_json_file.path().to_str().unwrap().to_owned();

        // Write a sample JSON file to import.
        fs::write(
            &tmp_json_file,
            r#"[{"name":"Alice","age":20,"optional":null},{"name":"Bob","age":25,"optional":0}]"#,
        )
        .unwrap();

        // Import the JSON file.
        assert!(super::import(tmp_db_filepath, "test", Some(tmp_json_file_path)).is_ok());

        // Check the imported data.
        let result = serde_json::to_string(
            &rusqlite::Connection::open(tmp_db_filepath)
                .unwrap()
                .prepare("SELECT name, age, optional FROM test")
                .unwrap()
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i32>(1)?,
                        row.get::<_, Option<i32>>(2)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
        )
        .unwrap();

        // key orders are not maintained
        assert_eq!(result, r#"[["Alice",20,null],["Bob",25,0]]"#);
    }

    #[test]
    fn test_empty_json() {
        let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

        let tmp_json_file = tempfile::NamedTempFile::new().unwrap();
        let tmp_json_file_path = tmp_json_file.path().to_str().unwrap().to_owned();

        // Write a sample JSON file to import.
        fs::write(&tmp_json_file, r#"[]"#).unwrap();

        // Import the JSON file.
        assert!(super::import(tmp_db_filepath, "test", Some(tmp_json_file_path))
            .unwrap_err()
            .to_string()
            .contains("No data present."));
    }
}
