use tempfile::{tempdir, NamedTempFile};

use crate::column_origin::{column_origin, ColumnOrigin};

#[test]
fn test_simple() {
    let f = NamedTempFile::new().unwrap();

    {
        let con = rusqlite::Connection::open(f.path()).unwrap();
        con.execute_batch(
            "
CREATE TABLE t1(c1 TEXT NOT NULL, c2 TEXT NOT NULL);
CREATE VIEW v1 AS SELECT c1 as c3, c2 FROM t1;
",
        )
        .unwrap();
    }

    assert_eq!(
        column_origin(f.path().to_str().unwrap(), "SELECT * FROM v1"),
        Ok(vec![
            ColumnOrigin {
                database: Some("main".to_owned()),
                table: Some("t1".to_owned()),
                column: Some("c1".to_owned()),
            },
            ColumnOrigin {
                database: Some("main".to_owned()),
                table: Some("t1".to_owned()),
                column: Some("c2".to_owned()),
            },
        ])
    );

    assert_eq!(
        column_origin(f.path().to_str().unwrap(), "SELECT 1, 2"),
        Ok(vec![
            ColumnOrigin {
                database: None,
                table: None,
                column: None,
            },
            ColumnOrigin {
                database: None,
                table: None,
                column: None,
            },
        ])
    );
}

#[test]
fn test_unable_to_open_database_file() {
    let dir = tempdir().unwrap();
    assert_eq!(
        column_origin(dir.path().to_str().unwrap(), "SELECT 1"),
        Err("Error opening database: unable to open database file".to_owned())
    );
}

#[test]
fn test_sqlite_prepare_error() {
    let f = NamedTempFile::new().unwrap();
    assert_eq!(
        column_origin(f.path().to_str().unwrap(), "SELEC"),
        Err("Error preparing statement: near \"SELEC\": syntax error".to_owned())
    );
}
