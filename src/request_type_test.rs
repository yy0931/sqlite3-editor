use crate::{
    literal::{Blob, Literal},
    request_type::{QueryMode, Request},
};

#[test]
fn test_request_encode_and_decode() {
    let data = Request {
        mode: QueryMode::ReadOnly,
        params: vec![Literal::Blob(Blob(vec![1, 2, 3]))],
        query: "query".to_owned(),
    };
    let msgpack = rmp_serde::to_vec(&data).unwrap();
    let decoded: Request = rmp_serde::from_slice(&msgpack).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_blob_request() {
    // "bin 8" should be decoded into Vec<u8>
    let decoded: Request = rmp_serde::from_slice(&[
        0x93, // fixarray len=3
        0xa1, // fixstr len=1
        0x61, // "a"
        0x91, // fixarray len=1
        0xc4, 0x3, 0xff, 0xef, 0xdf, // bin 8 len=3
        0xaa, // fixstr len=10
        0x72, 0x65, 0x61, 0x64, 0x5f, 0x77, 0x72, 0x69, 0x74, 0x65, // "read_write"
    ])
    .unwrap();
    assert_eq!(
        decoded,
        Request {
            query: "a".to_owned(),
            params: vec![Literal::Blob(Blob(vec![255, 239, 223]))],
            mode: QueryMode::ReadWrite,
        }
    );
}
