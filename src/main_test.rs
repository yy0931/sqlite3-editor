use std::io::Cursor;

use crate::{cli, Args, Query};

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
