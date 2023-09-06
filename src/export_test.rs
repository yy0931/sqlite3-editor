use std::io::Read;

use crate::{export::export, FileFormat};

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
    assert_eq!(buf, "t,i,n,r,b\nAlice,20,,1.2,AQID\nAlice,25,,2.4,BAUG\n");
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
    assert_eq!(buf, "t;i;n;r;b\nAlice;20;;1.2;AQID\nAlice;25;;2.4;BAUG\n");
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
    assert_eq!(buf, "t\ti\tn\tr\tb\nAlice\t20\t\t1.2\tAQID\nAlice\t25\t\t2.4\tBAUG\n");
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
    assert_eq!(
        buf,
        r#"[{"t":"Alice","i":20,"n":null,"r":1.2,"b":"AQID"},{"t":"Alice","i":25,"n":null,"r":2.4,"b":"BAUG"}]"#
    );
}

#[test]
fn test_invalid_delimiter() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

    let tmp_out = tempfile::NamedTempFile::new().unwrap();
    let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

    setup_test_db(&tmp_db_filepath);

    assert!(export(
        &tmp_db_filepath,
        &None,
        "SELECT * FROM test",
        FileFormat::JSON,
        ",,",
        Some(tmp_out_path),
    )
    .unwrap_err()
    .to_string()
    .contains("csv_delimiter needs to be a single character."));
}
