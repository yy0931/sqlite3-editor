use std::collections::HashMap;

use crate::column_origin::{column_origin, ColumnOrigin};

#[test]
fn test_simple() {
    let con = rusqlite::Connection::open_in_memory().unwrap();

    con.execute_batch(
        "
CREATE TABLE t1(c1 TEXT NOT NULL, c2 TEXT NOT NULL);
CREATE VIEW v1 AS SELECT c1 as c3, c2 FROM t1;
",
    )
    .unwrap();

    assert_eq!(
        column_origin(unsafe { con.handle() }, "SELECT * FROM v1"),
        Ok(HashMap::from([
            ("c3".to_owned(), ColumnOrigin::new("main", "t1", "c1")),
            ("c2".to_owned(), ColumnOrigin::new("main", "t1", "c2")),
        ]))
    );

    assert_eq!(
        column_origin(unsafe { con.handle() }, "SELECT 1, 2"),
        Ok(HashMap::new()),
    );
}

#[test]
fn test_sqlite_prepare_error() {
    let con = rusqlite::Connection::open_in_memory().unwrap();
    assert_eq!(
        column_origin(unsafe { con.handle() }, "SELEC"),
        Err("Error preparing statement: near \"SELEC\": syntax error".to_owned())
    );
}
