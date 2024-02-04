use std::{rc::Rc, time::Duration};

use tempfile::NamedTempFile;

use crate::cache::{cache_entry::Records, pager::Pager};

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
    assert_eq!(
        &result1.col_buf(),
        &[vec![0xa1, b'a', 0xa1, b'c'], vec![0xa1, b'b', 0xa1, b'd'],]
    );
    assert_eq!(result1.n_rows(), 2);
    assert_eq!(result1.columns(), Rc::new(vec!["x".to_owned(), "y".to_owned()]));
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

    assert_eq!(
        result2,
        Records::new(
            vec![vec![0, 2, 4], vec![1, 3, 5]],
            3,
            Rc::new(vec!["x".into(), "y".into()])
        )
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
        pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap().n_rows(),
        3
    );

    // cache hit
    assert_eq!(
        pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap().n_rows(),
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
        pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap().n_rows(),
        5
    );

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[10.into(), 10.into()];
    assert_eq!(
        pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap().n_rows(),
        0
    );

    let query = "SELECT * FROM t LIMIT ? OFFSET ?";
    let params = &[10.into(), 3.into()];
    assert_eq!(
        pager.query(&mut conn, query, params, |_| {}).unwrap().unwrap().n_rows(),
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
