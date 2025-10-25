use std::rc::Rc;
use std::time::Duration;

use tempfile::NamedTempFile;

use crate::cli_error::CLIError;
use crate::cli_error::CLIErrorCode;
use crate::cli_subcommands::server::server_commands::query::sqlite3::cache::Pager;
use crate::cli_subcommands::server::server_commands::query::sqlite3::cache::Records;
use crate::msgpack::explain_msgpack;
use crate::msgpack::MessagePackArray;

#[test]
fn test_repeat_same_query() {
    // Setup
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t(x, y)", ()).unwrap();
    conn.execute("INSERT INTO t VALUES (?, ?), (?, ?)", ("a", "b", "c", "d"))
        .unwrap();
    let mut pager = Pager::new();
    pager.config.slow_query_threshold = Duration::ZERO;
    pager.config.cache_time_limit_relative_to_queried_range = f64::MAX;
    assert!(pager.total_cache_size_bytes() == 0);

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[3.into(), 0.into()];

    // Select 1
    let result1 = pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap();
    assert_eq!(pager.cache_hit_count, 0);

    // Select 2
    let result2 = pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap();
    assert_eq!(pager.cache_hit_count, 1);

    // Compare records
    assert_eq!(&result1, &result2);
    assert_eq!(result1.col_buf().len(), 2);
    assert_eq!(
        explain_msgpack(result1.col_buf()[0].to_vec()),
        "fixarray(2) fixstr(1) 97 fixstr(1) 99"
    );
    assert_eq!(
        explain_msgpack(result1.col_buf()[1].to_vec()),
        "fixarray(2) fixstr(1) 98 fixstr(1) 100"
    );
    assert_eq!(result1.columns(), Rc::new(vec!["x".to_owned(), "y".to_owned()]));
    assert!(pager.total_cache_size_bytes() > 0);
}

#[test]
fn test_backward_cache() {
    // Setup
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t(x, y)", ()).unwrap();
    for i in (0..18).step_by(2) {
        conn.execute("INSERT INTO t VALUES (?, ?)", (i, i + 1)).unwrap();
    }
    let mut pager = Pager::new();
    pager.config.slow_query_threshold = Duration::ZERO;
    pager.config.cache_time_limit_relative_to_queried_range = f64::MAX;

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";

    // Select 1
    pager
        .query(&mut conn, query, &[3.into(), 3.into()], |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(pager.cache_hit_count, 0);

    // Select 2
    let result2 = pager
        .query(&mut conn, query, &[3.into(), 0.into()], |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(pager.cache_hit_count, 1);

    let mut arr1 = MessagePackArray::new();
    arr1.push_message_pack(vec![0]); // 0XXXXXXX = positive fixint
    arr1.push_message_pack(vec![2]);
    arr1.push_message_pack(vec![4]);
    let mut arr2 = MessagePackArray::new();
    arr2.push_message_pack(vec![1]);
    arr2.push_message_pack(vec![3]);
    arr2.push_message_pack(vec![5]);
    assert_eq!(
        result2,
        Records::new(vec![arr1, arr2], Rc::new(vec!["x".into(), "y".into()]))
    );
}

#[test]
fn test_cache_limit_bytes() {
    // Setup
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t(x, y)", ()).unwrap();
    for i in (0..18).step_by(2) {
        conn.execute("INSERT INTO t VALUES (?, ?)", (i, i + 1)).unwrap();
    }
    let mut pager = Pager::new();
    pager.config.slow_query_threshold = Duration::ZERO;
    pager.config.cache_time_limit_relative_to_queried_range = f64::MAX;
    pager.config.cache_limit_bytes = 0;

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";

    // Select 1
    pager
        .query(&mut conn, query, &[3.into(), 3.into()], |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(pager.dequeue_count, 0);

    // Select 2
    pager
        .query(&mut conn, query, &[3.into(), 0.into()], |_| {})
        .unwrap()
        .unwrap();
    assert_eq!(pager.dequeue_count, 1);
}

#[test]
fn test_data_version() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x, y)", ()).unwrap();
    conn.execute("INSERT INTO t VALUES (?, ?), (?, ?)", (1, 2, 3, 4))
        .unwrap();

    let mut pager = Pager::new();
    pager.config.slow_query_threshold = Duration::ZERO;
    pager.config.cache_time_limit_relative_to_queried_range = f64::MAX;

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[3.into(), 0.into()];

    pager.query(&mut conn, query, params, |_| {}).unwrap();
    assert_eq!(pager.data_version(), Some(1));

    conn.execute("INSERT INTO t VALUES (1, 2)", ()).unwrap();
    pager.query(&mut conn, query, params, |_| {}).unwrap();

    assert_eq!(pager.data_version(), Some(1));

    std::thread::spawn(move || {
        let conn = rusqlite::Connection::open(f.path()).unwrap();
        conn.execute("INSERT INTO t VALUES (1, 2)", ()).unwrap();
    })
    .join()
    .unwrap();

    pager.query(&mut conn, query, params, |_| {}).unwrap();
    assert_eq!(pager.data_version(), Some(2));
}

fn get_num_rows(records: &Records) -> usize {
    records.col_buf().first().unwrap().len()
}

#[test]
fn test_cache_hit_with_unknown_num_records() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x)", ()).unwrap();
    for i in 0..10 {
        conn.execute("INSERT INTO t VALUES (?)", (i,)).unwrap();
    }

    let mut pager = Pager::new();
    pager.config.margin_start = 0;
    pager.config.margin_end = 0;

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[3.into(), 1.into()];
    assert_eq!(
        get_num_rows(&pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap()),
        3
    );

    // cache hit
    assert_eq!(
        get_num_rows(&pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap()),
        3
    );
}

#[test]
fn test_out_of_bounds() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x)", ()).unwrap();
    for i in 0..5 {
        conn.execute("INSERT INTO t VALUES (?)", (i,)).unwrap();
    }

    let mut pager = Pager::new();

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[10.into(), 0.into()];
    assert_eq!(
        get_num_rows(&pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap()),
        5
    );

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[10.into(), 10.into()];
    assert_eq!(
        get_num_rows(&pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap()),
        0
    );

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[10.into(), 3.into()];
    assert_eq!(
        get_num_rows(&pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap()),
        2
    );
}

#[test]
fn test_query_error() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    let mut pager = Pager::new();

    let query = r#"SELECT * FROM "non-existent-table" LIMIT ? OFFSET ?"#;
    let params = &[10.into(), 0.into()];
    pager.query(&mut conn, query, params, |_| {}).unwrap_err();
}

#[test]
fn test_negative_limit() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x)", ()).unwrap();

    let mut pager = Pager::new();

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[(-1).into(), 0.into()];
    assert_eq!(pager.query(&mut conn, query, params, |_| {}), Ok(None));
}

#[test]
fn test_negative_offset() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x)", ()).unwrap();

    let mut pager = Pager::new();

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[0.into(), (-1).into()];
    assert_eq!(pager.query(&mut conn, query, params, |_| {}), Ok(None));
}

#[test]
fn test_wrong_parameter_type() {
    let f = NamedTempFile::new().unwrap();

    let mut conn = rusqlite::Connection::open(f.path()).unwrap();
    conn.execute("CREATE TABLE t(x)", ()).unwrap();

    let mut pager = Pager::new();

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &["1".into(), "1".into()];
    assert_eq!(pager.query(&mut conn, query, params, |_| {}), Ok(None));
}

#[test]
fn test_failed_to_start_a_transaction() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute("CREATE TABLE t(x)", ()).unwrap();
    conn.execute("BEGIN", ()).unwrap();

    let mut pager = Pager::new();

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &["1".into(), "1".into()];
    assert_eq!(
        pager.query(&mut conn, query, params, |_| {}),
        Err(CLIError::Query {
            message: "cannot start a transaction within a transaction".to_owned(),
            query: "BEGIN;".to_owned(),
            params: vec![],
            code: CLIErrorCode::OtherError
        })
    );
}
