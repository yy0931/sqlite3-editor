use tempfile::NamedTempFile;

use crate::column_origin::{column_origin, ColumnOrigin};

#[test]
fn test() {
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
}
