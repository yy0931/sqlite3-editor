use crate::{
    literal::{Blob, Literal},
    request_type::{QueryMode, Request},
    sqlite3::QueryOptions,
};

#[test]
fn test_request_encode_and_decode() {
    let data = Request {
        mode: QueryMode::ReadOnly,
        params: vec![Literal::Blob(Blob(vec![1, 2, 3]))],
        query: "query".to_owned(),
        options: QueryOptions::default(),
    };
    let msgpack = rmp_serde::to_vec(&data).unwrap();
    let decoded: Request = rmp_serde::from_slice(&msgpack).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_request_encode_and_decode_with_changes() {
    let data = Request {
        mode: QueryMode::ReadWrite,
        params: vec![Literal::Blob(Blob(vec![1, 2, 3]))],
        query: "query".to_owned(),
        options: QueryOptions {
            changes: Some(5),
            ..Default::default()
        },
    };
    let msgpack = rmp_serde::to_vec(&data).unwrap();
    let decoded: Request = rmp_serde::from_slice(&msgpack).unwrap();
    assert_eq!(decoded, data);
}
