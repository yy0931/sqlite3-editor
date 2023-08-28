use std::io::Read;

use crate::{export::export, FileFormat};

fn setup_test_db(tmp_db_filepath: &str) {
    let connection = rusqlite::Connection::open(tmp_db_filepath).unwrap();

    connection
        .execute("CREATE TABLE test (name TEXT NOT NULL, age INTEGER NOT NULL);", [])
        .unwrap();

    connection
        .execute("INSERT INTO test (name, age) VALUES (?, ?)", ["Alice", "20"])
        .unwrap();

    connection
        .execute("INSERT INTO test (name, age) VALUES (?, ?)", ["Bob", "25"])
        .unwrap();
}

#[test]
fn test_export_csv() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

    let mut tmp_out = tempfile::NamedTempFile::new().unwrap();
    let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

    setup_test_db(&tmp_db_filepath);

    export(
        &tmp_db_filepath,
        &None,
        "SELECT * FROM test",
        FileFormat::CSV,
        ",",
        Some(tmp_out_path),
    )
    .unwrap();

    let mut buf = String::new();
    tmp_out.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, "name,age\nAlice,20\nBob,25\n");
}

#[test]
fn test_export_csv_delimiter() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

    let mut tmp_out = tempfile::NamedTempFile::new().unwrap();
    let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

    setup_test_db(&tmp_db_filepath);

    export(
        &tmp_db_filepath,
        &None,
        "SELECT * FROM test",
        FileFormat::CSV,
        ";",
        Some(tmp_out_path),
    )
    .unwrap();

    let mut buf = String::new();
    tmp_out.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, "name;age\nAlice;20\nBob;25\n");
}

#[test]
fn test_export_tsv() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

    let mut tmp_out = tempfile::NamedTempFile::new().unwrap();
    let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

    setup_test_db(&tmp_db_filepath);

    export(
        &tmp_db_filepath,
        &None,
        "SELECT * FROM test",
        FileFormat::TSV,
        ",",
        Some(tmp_out_path),
    )
    .unwrap();

    let mut buf = String::new();
    tmp_out.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, "name\tage\nAlice\t20\nBob\t25\n");
}

#[test]
fn test_export_json() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

    let mut tmp_out = tempfile::NamedTempFile::new().unwrap();
    let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

    setup_test_db(&tmp_db_filepath);

    export(
        &tmp_db_filepath,
        &None,
        "SELECT * FROM test",
        FileFormat::JSON,
        ",",
        Some(tmp_out_path),
    )
    .unwrap();

    let mut buf = String::new();
    tmp_out.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, r#"[{"name":"Alice","age":20},{"name":"Bob","age":25}]"#);
}
