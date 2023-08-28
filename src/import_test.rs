use crate::import::import;
use crate::FileFormat;
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
    import(
        tmp_db_filepath,
        &None,
        FileFormat::CSV,
        "test",
        ",",
        Some(tmp_csv_file_path.to_string()),
    )
    .unwrap();

    // Check the imported data.
    assert_eq!(
        serde_json::to_string(
            &rusqlite::Connection::open(tmp_db_filepath)
                .unwrap()
                .prepare("SELECT * FROM test")
                .unwrap()
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        )
        .unwrap(),
        r#"[["Alice","20"],["Bob","25"]]"#
    );
}

#[test]
fn test_import_csv_delimiter() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

    let mut tmp_csv_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_csv_file_path = tmp_csv_file.path().to_str().unwrap().to_owned();

    // Write a sample CSV file to import.
    writeln!(tmp_csv_file, "name;age\nAlice;20\nBob;25").unwrap();

    // Import the CSV file.
    import(
        tmp_db_filepath,
        &None,
        FileFormat::CSV,
        "test",
        ";",
        Some(tmp_csv_file_path.to_string()),
    )
    .unwrap();

    // Check the imported data.
    assert_eq!(
        serde_json::to_string(
            &rusqlite::Connection::open(tmp_db_filepath)
                .unwrap()
                .prepare("SELECT * FROM test")
                .unwrap()
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        )
        .unwrap(),
        r#"[["Alice","20"],["Bob","25"]]"#
    );
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
    import(
        tmp_db_filepath,
        &None,
        FileFormat::TSV,
        "test",
        ",",
        Some(tmp_csv_file_path.to_string()),
    )
    .unwrap();

    // Check the imported data.
    assert_eq!(
        serde_json::to_string(
            &rusqlite::Connection::open(tmp_db_filepath)
                .unwrap()
                .prepare("SELECT * FROM test")
                .unwrap()
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        )
        .unwrap(),
        r#"[["Alice","20"],["Bob","25"]]"#
    );
}

#[test]
fn test_import_json() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap();

    let tmp_json_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_json_file_path = tmp_json_file.path().to_str().unwrap().to_owned();

    // Write a sample JSON file to import.
    fs::write(&tmp_json_file, r#"[{"name":"Alice","age":20},{"name":"Bob","age":25}]"#).unwrap();

    // Import the JSON file.
    assert!(import(
        tmp_db_filepath,
        &None,
        FileFormat::JSON,
        "test",
        ",",
        Some(tmp_json_file_path.clone())
    )
    .is_ok());

    // Check the imported data.
    let result = serde_json::to_string(
        &rusqlite::Connection::open(tmp_db_filepath)
            .unwrap()
            .prepare("SELECT * FROM test")
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
    )
    .unwrap();

    // key orders are not maintained
    assert!(result == r#"[["Alice","20"],["Bob","25"]]"# || result == r#"[["20","Alice"],["25","Bob"]]"#);
}
