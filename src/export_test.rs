use crate::export;

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
    let mut buf = Vec::new();

    setup_test_db(&tmp_db_filepath);

    export::export_csv(&tmp_db_filepath, &None, "SELECT * FROM test", ",", &mut buf).unwrap();
    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "t,i,n,r,b\nAlice,20,,1.2,AQID\nAlice,25,,2.4,BAUG\n"
    );
}

#[test]
fn test_export_tsv() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
    let mut buf = Vec::new();

    setup_test_db(&tmp_db_filepath);

    export::export_csv(&tmp_db_filepath, &None, "SELECT * FROM test", "\t", &mut buf).unwrap();

    assert_eq!(
        String::from_utf8(buf).unwrap(),
        "t\ti\tn\tr\tb\nAlice\t20\t\t1.2\tAQID\nAlice\t25\t\t2.4\tBAUG\n"
    );
}

#[test]
fn test_export_json() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
    let mut buf = Vec::new();

    setup_test_db(&tmp_db_filepath);

    export::export_json(&tmp_db_filepath, &None, "SELECT * FROM test", &mut buf).unwrap();

    assert_eq!(
        String::from_utf8(buf).unwrap(),
        r#"[{"t":"Alice","i":20,"n":null,"r":1.2,"b":"AQID"},{"t":"Alice","i":25,"n":null,"r":2.4,"b":"BAUG"}]"#
    );
}

#[test]
fn test_invalid_delimiter() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();
    let mut buf = Vec::new();

    setup_test_db(&tmp_db_filepath);

    assert!(
        export::export_csv(&tmp_db_filepath, &None, "SELECT * FROM test", ",,", &mut buf)
            .unwrap_err()
            .to_string()
            .contains("csv_delimiter needs to be a single character.")
    );
}

#[test]
fn test_export_xlsx() {
    let tmp_db_file = tempfile::NamedTempFile::new().unwrap();
    let tmp_db_filepath = tmp_db_file.path().to_str().unwrap().to_owned();

    let tmp_out = tempfile::NamedTempFile::new().unwrap();
    let tmp_out_path = tmp_out.path().to_str().unwrap().to_owned();

    setup_test_db(&tmp_db_filepath);

    export::export_xlsx(&tmp_db_filepath, &None, "SELECT * FROM test", &tmp_out_path).unwrap();

    dbg!(tmp_out.as_file().metadata().unwrap().len());
}
