use crate::{
    literal::Literal,
    request_type::{QueryMode, Request},
    sqlite3::QueryOptions,
};

#[test]
fn test_value() {
    use std::collections::HashMap;
    let value: HashMap<&str, Literal> = serde_json::from_str(r#"{"a": 10, "b": null}"#).unwrap();
    assert_eq!(value, HashMap::from([("a", Literal::I64(10)), ("b", Literal::Nil)]));
}

#[test]
fn test_parse_query() {
    let q: Request =
        serde_json::from_str(r#"["foo", [1, 2], "read_only", {"changes": null, "allow_fewer_changes": false}]"#)
            .unwrap();
    assert_eq!(
        q,
        Request {
            query: "foo".to_owned(),
            params: vec![Literal::I64(1), Literal::I64(2)],
            mode: QueryMode::ReadOnly,
            options: QueryOptions::default(),
        }
    );
}

#[test]
fn test_parse_query_with_changes() {
    let q: Request =
        serde_json::from_str(r#"["foo", [1, 2], "read_only", {"changes": 10, "allow_fewer_changes": true}]"#).unwrap();
    assert_eq!(
        q,
        Request {
            query: "foo".to_owned(),
            params: vec![Literal::I64(1), Literal::I64(2)],
            mode: QueryMode::ReadOnly,
            options: QueryOptions {
                changes: Some(10),
                allow_fewer_changes: true,
                ..Default::default()
            },
        }
    );
}

#[test]
fn test_query_mode() {
    let value: QueryMode = serde_json::from_str(r#""read_only""#).unwrap();
    assert_eq!(value, QueryMode::ReadOnly);
    assert_eq!(serde_json::to_string(&QueryMode::ReadWrite).unwrap(), r#""read_write""#);
}
