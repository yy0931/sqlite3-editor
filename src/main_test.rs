use std::io::Cursor;

use tempfile::NamedTempFile;

use crate::{cli, Args, ExportFormat, Query};

#[test]
fn test_parse_query() {
    let q: Query = serde_json::from_str("[\"foo\"]").unwrap();
    assert_eq!(
        q,
        Query {
            query: "foo".to_owned()
        }
    );
}

#[test]
fn test_version() {
    let mut stdout = vec![];
    assert_eq!(
        cli(
            Args {
                command: crate::Commands::Version {},
            },
            || Cursor::new("".to_owned()),
            &mut stdout,
            &mut vec![],
        ),
        0
    );
    assert!(String::from_utf8(stdout).unwrap().starts_with("sqlite3-editor "));
}

#[test]
fn test_function_list() {
    let mut stdout = vec![];
    assert_eq!(
        cli(
            Args {
                command: crate::Commands::FunctionList {},
            },
            || Cursor::new("".to_owned()),
            &mut stdout,
            &mut vec![],
        ),
        0
    );
    let json = String::from_utf8(stdout).unwrap();
    let json = json.trim();
    assert!(json.starts_with("[\""));
    assert!(json.ends_with("\"]"));
}

fn test_export_to_stdout(format: ExportFormat) -> String {
    let f = NamedTempFile::new().unwrap();

    let conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x, y)", ()).unwrap();
    conn.execute("INSERT INTO t VALUES (1, 2), (3, 4)", ()).unwrap();

    let mut stdout = vec![];
    assert_eq!(
        cli(
            Args {
                command: crate::Commands::Export {
                    database_filepath: f.path().to_str().unwrap().to_owned(),
                    sql_cipher_key: None,
                    format,
                    query: "SELECT * FROM t".to_owned(),
                    csv_delimiter: ",".to_owned(),
                    output_file: None,
                },
            },
            || Cursor::new("".to_owned()),
            &mut stdout,
            &mut vec![],
        ),
        0
    );
    String::from_utf8(stdout).unwrap()
}

#[test]
fn test_export_csv() {
    assert_eq!(test_export_to_stdout(ExportFormat::CSV), "x,y\n1,2\n3,4\n");
}

#[test]
fn test_export_tsv() {
    assert_eq!(test_export_to_stdout(ExportFormat::TSV), "x\ty\n1\t2\n3\t4\n");
}

#[test]
fn test_export_json() {
    assert_eq!(
        test_export_to_stdout(ExportFormat::JSON),
        "[{\"x\":1,\"y\":2},{\"x\":3,\"y\":4}]"
    );
}
