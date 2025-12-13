use std::io::Write;

/// Performs a health check using an in-memory database, then writes the version information to the provided writer.
///
/// Output:
/// - the extension's version
/// - the bundled SQLite version
/// - the bundled SQLite's compile-time options
pub fn run(mut writer: &mut impl Write) {
    let con = rusqlite::Connection::open_in_memory().unwrap();
    con.execute("CREATE TABLE t(v)", ()).unwrap();
    con.execute("INSERT INTO t VALUES (?)", ["ok"]).unwrap();
    assert_eq!(con.query_row("SELECT v FROM t", [], |row| row.get::<_, String>(0)).unwrap(), "ok");

    writeln!(&mut writer, "sqlite3-editor {}", env!("CARGO_PKG_VERSION")).expect("writeln! failed.");
    writeln!(&mut writer, "SQLite {}", rusqlite::version()).expect("writeln! failed.");

    let conn = rusqlite::Connection::open_in_memory().unwrap();

    writeln!(
        &mut writer,
        "\nCompile options:\n{}",
        conn.prepare("PRAGMA compile_options")
            .unwrap()
            .query_map((), |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
            .into_iter()
            .map(|line| "- ".to_owned() + &line)
            .collect::<Vec<_>>()
            .join("\n")
    )
    .expect("writeln! failed.");
}

#[cfg(test)]
mod test {
    #[test]
    fn test_version() {
        let mut stdout = vec![];
        super::run(&mut stdout);
        assert!(String::from_utf8(stdout).unwrap().starts_with("sqlite3-editor "));
    }
}
