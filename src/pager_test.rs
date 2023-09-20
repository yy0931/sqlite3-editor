use std::time::Duration;

use tempfile::NamedTempFile;

use crate::pager::{Pager, Records};

#[test]
fn test_repeat_same_query() -> () {
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
        &result1.col_buf,
        &[
            vec![0xa1, 'a' as u8, 0xa1, 'c' as u8],
            vec![0xa1, 'b' as u8, 0xa1, 'd' as u8],
        ]
    );
    assert_eq!(result1.n_rows, 2);
    assert_eq!(result1.columns, vec!["x", "y"]);
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
        Records {
            col_buf: vec![vec![0, 2, 4], vec![1, 3, 5],],
            n_rows: 3,
            columns: vec!["x".into(), "y".into()],
        }
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
