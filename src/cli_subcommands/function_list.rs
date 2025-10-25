use std::io::Write;

/// Writes the list of functions included in the bundled SQLite to the provided writer.
///
/// Output: string[]
pub fn run(mut writer: &mut impl Write) {
    let mut functions = rusqlite::Connection::open_in_memory()
        .unwrap()
        .prepare("SELECT DISTINCT name FROM pragma_function_list()")
        .unwrap()
        .query_map((), |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    functions.sort();
    writeln!(&mut writer, "{}", serde_json::to_string(&functions).unwrap()).expect("writeln! failed.");
}

#[cfg(test)]
mod test {
    #[test]
    fn test_function_list() {
        let mut stdout = vec![];
        super::run(&mut stdout);
        let json = String::from_utf8(stdout).unwrap();
        let json = json.trim();
        assert!(json.starts_with("[\""));
        assert!(json.ends_with("\"]"));
    }
}
