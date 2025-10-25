use crate::cli_error::CLIError;
use crate::cli_subcommands::server::sqlite3_fns::assert_readonly_query::assert_readonly_query;
use crate::cli_value::CLIValue;

/// Executes a SQL statement and maps the result.
pub fn select_all<F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>, T>(
    conn: &rusqlite::Connection,
    query: &str,
    params: &[CLIValue],
    mut map: F,
) -> std::result::Result<Vec<T>, CLIError> {
    assert_readonly_query(query, &None)?;

    // Prepare the statement
    let mut stmt = conn
        .prepare(query)
        .or_else(|err| CLIError::new_query_error(err, query, params))?;

    // Bind parameters
    for (i, param) in params.iter().enumerate() {
        stmt.raw_bind_parameter(i + 1, param)
            .or_else(|err| CLIError::new_query_error(err, query, params))?;
    }

    // Fetch data and pack into Vec<T>
    let mut rows = stmt.raw_query();
    let mut records = Vec::<T>::new();
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                records.push(map(row).or_else(|err| CLIError::new_query_error(err, query, params))?);
            }
            Ok(None) => break,
            Err(err) => return Err(CLIError::new_query_error(err, query, params)?),
        }
    }

    Ok(records)
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use crate::cli_subcommands::server::server_commands::query::request_type::QueryMode;
    use crate::cli_subcommands::server::server_commands::query::request_type::QueryOptions;
    use crate::cli_subcommands::server::server_commands::query::sqlite3::connection::ExecMode;
    use crate::cli_subcommands::server::server_commands::query::sqlite3::connection::SQLite3Connection;
    use crate::cli_subcommands::server::sqlite3_fns::select_all::select_all;
    use crate::cli_value::CLIValue;

    #[test]
    fn test_select_params() {
        let db = SQLite3Connection::connect(":memory:", false).unwrap();

        let params: Vec<CLIValue> = vec![CLIValue::Nil, "a".into(), 123.into(), 1.23.into(), vec![1, 2, 3].into()];
        assert_eq!(
            select_all(db.con(), "SELECT ?, ?, ?, ?, ?", &params, |v| {
                Ok((
                    CLIValue::from(v.get_ref_unwrap(0)),
                    CLIValue::from(v.get_ref_unwrap(1)),
                    CLIValue::from(v.get_ref_unwrap(2)),
                    CLIValue::from(v.get_ref_unwrap(3)),
                    CLIValue::from(v.get_ref_unwrap(4)),
                ))
            })
            .unwrap()
            .first()
            .unwrap(),
            &(
                params[0].to_owned(),
                params[1].to_owned(),
                params[2].to_owned(),
                params[3].to_owned(),
                params[4].to_owned()
            )
        );
    }

    #[test]
    fn test_json() {
        let db = SQLite3Connection::connect(":memory:", false).unwrap();
        assert_eq!(
            select_all(
                db.con(),
                r#"select json_extract('{"foo": {"bar": 123}}', '$.foo.bar');"#,
                &[],
                |row| row.get::<_, i64>(0)
            )
            .unwrap()
            .first()
            .unwrap(),
            &123
        );
    }

    #[test]
    fn test_transaction_success() {
        let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
        db.handle(
            &mut Cursor::new(Vec::<u8>::new()),
            r#"
BEGIN;
CREATE TABLE t(x);
INSERT INTO t VALUES (1);
COMMIT;"#,
            &[],
            QueryMode::Script,
            QueryOptions::default(),
        )
        .unwrap();

        assert_eq!(
            select_all(db.con(), "SELECT * FROM t", &[], |row| row.get::<_, i64>(0))
                .unwrap()
                .first()
                .unwrap(),
            &1
        );
    }

    #[test]
    fn test_transaction_rollback() {
        let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
        match db.handle(
            &mut Cursor::new(Vec::<u8>::new()),
            r#"
-- prepare
CREATE TABLE t(x);
CREATE VIEW IF NOT EXISTS invalid_view AS SELECT * FROM non_existent;

-- Begin a transaction and raise an error.
BEGIN;
INSERT INTO t VALUES (1);
SELECT * FROM invalid_view;
COMMIT;"#,
            &[],
            QueryMode::Script,
            QueryOptions::default(),
        ) {
            Ok(_) => panic!(),
            Err(err) => {
                if !err.to_string().contains("no such table") {
                    panic!("{err}");
                }
            }
        }

        match db.handle(
            &mut Cursor::new(Vec::<u8>::new()),
            r#"
-- The previous transaction should have been aborted.
BEGIN;
INSERT INTO t VALUES (1);
SELECT * FROM invalid_view;
COMMIT;"#,
            &[],
            QueryMode::Script,
            QueryOptions::default(),
        ) {
            Ok(_) => panic!(),
            Err(err) => {
                if !err.to_string().contains("no such table") {
                    panic!("{err}");
                }
            }
        }

        assert_eq!(
            select_all(db.con(), "SELECT count(*) FROM t", &[], |row| row.get::<_, i64>(0))
                .unwrap()
                .first()
                .unwrap(),
            &0
        );
    }

    #[test]
    fn test_uncommitted_transaction() {
        let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
        db.handle(
            &mut Cursor::new(Vec::<u8>::new()),
            r#"
CREATE TABLE t(x);
BEGIN;
INSERT INTO t VALUES (1);
"#,
            &[],
            QueryMode::Script,
            QueryOptions::default(),
        )
        .unwrap();

        db.handle(
            &mut Cursor::new(Vec::<u8>::new()),
            "BEGIN;",
            &[],
            QueryMode::Script,
            QueryOptions::default(),
        )
        .unwrap();

        assert_eq!(
            select_all(db.con(), "SELECT count(*) FROM t", &[], |row| row.get::<_, i64>(0))
                .unwrap()
                .first()
                .unwrap(),
            &0
        );
    }

    #[test]
    fn test_changes() {
        let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
        db._execute(
            "CREATE TABLE t(x INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT)",
            &[],
            ExecMode::ReadWrite,
            QueryOptions::default(),
            &mut vec![],
        )
        .unwrap();

        assert_eq!(
            select_all(db.con(), "SELECT * FROM t", &[], |_| Ok(1)).unwrap().len(),
            0
        );

        assert_eq!(
            db._execute(
                "INSERT INTO t DEFAULT VALUES",
                &[],
                ExecMode::ReadWrite,
                QueryOptions {
                    changes: Some(2),
                    ..Default::default()
                },
                &mut vec![],
            ),
            Err(crate::cli_error::CLIError::UnexpectedChanges {
                expected: 2,
                actual: 1,
                query: "INSERT INTO t DEFAULT VALUES".to_owned(),
                params: vec![],
            }),
        );

        assert_eq!(
            select_all(db.con(), "SELECT * FROM t", &[], |_| Ok(1)).unwrap().len(),
            0
        );

        db._execute(
            "INSERT INTO t DEFAULT VALUES",
            &[],
            ExecMode::ReadWrite,
            QueryOptions::default(),
            &mut vec![],
        )
        .unwrap();

        assert_eq!(
            select_all(db.con(), "SELECT * FROM t", &[], |_| Ok(1)).unwrap().len(),
            1
        );

        db._execute(
            "INSERT INTO t DEFAULT VALUES",
            &[],
            ExecMode::ReadWrite,
            QueryOptions {
                changes: Some(1),
                ..Default::default()
            },
            &mut vec![],
        )
        .unwrap();

        assert_eq!(
            select_all(db.con(), "SELECT * FROM t", &[], |_| Ok(1)).unwrap().len(),
            2
        );
    }

    #[test]
    fn test_pre_stmt() {
        let mut db = SQLite3Connection::connect(":memory:", false).unwrap();
        db._execute(
            "CREATE TABLE a AS SELECT 1",
            &[],
            ExecMode::ReadWrite,
            QueryOptions {
                pre_stmt: Some("CREATE TABLE b AS SELECT 1".to_owned()),
                ..QueryOptions::default()
            },
            &mut vec![],
        )
        .unwrap();
        select_all(db.con(), "SELECT * FROM a", &[], |_| Ok(1)).unwrap();
        select_all(db.con(), "SELECT * FROM b", &[], |_| Ok(1)).unwrap();
    }
}
