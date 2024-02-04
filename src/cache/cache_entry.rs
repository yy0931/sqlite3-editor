use std::{collections::HashMap, rc::Rc, time::Instant};

use crate::{literal::Literal, util::into};

#[derive(Debug, PartialEq, Eq)]
pub struct Records {
    col_buf: Vec<Vec<u8>>,
    // The max number of elements in an array in msgpack is u32::MAX.
    n_rows: u32,
    columns: Rc<Vec<String>>,
}

impl Records {
    pub fn new(col_buf: Vec<Vec<u8>>, n_rows: u32, columns: Rc<Vec<String>>) -> Self {
        Self {
            col_buf,
            n_rows,
            columns,
        }
    }
    pub fn col_buf(&self) -> &[Vec<u8>] {
        &self.col_buf
    }
    pub fn n_rows(&self) -> u32 {
        self.n_rows
    }
    pub fn columns(&self) -> Rc<Vec<String>> {
        Rc::clone(&self.columns)
    }
}

/// Stores the last result of a query.
pub(super) struct PagerCacheEntry {
    query: String,
    params: Vec<Literal>,

    records: HashMap</* offset */ u64, Vec<u8>>,
    columns: Option<Rc<Vec<String>>>,

    /// The total number of records to be returned when querying without a LIMIT and OFFSET. 'None' indicates unknown.
    num_records: Option<u64>,

    last_accessed: Instant,

    // The approximate size (stack size + heap size) of this struct in bytes.
    total_size_bytes: u64,
}

impl PagerCacheEntry {
    pub(super) fn new(
        query: String,
        params: Vec<Literal>,
        records: HashMap<u64, Vec<u8>>,
        columns: Option<Vec<String>>,
        num_records: Option<u64>,
    ) -> Self {
        Self {
            total_size_bytes: into::<_, u64>(std::mem::size_of::<Self>())
                + into::<_, u64>(query.capacity())
                + params
                    .iter()
                    .map(|p| {
                        into::<_, u64>(std::mem::size_of_val(p))
                            + match p {
                                Literal::Bool(_) | Literal::F64(_) | Literal::I64(_) | Literal::Nil => 0u64,
                                Literal::Blob(b) => into::<_, u64>(std::mem::size_of_val(b) + b.0.capacity()),
                                Literal::String(s) => into::<_, u64>(s.capacity()),
                            }
                    })
                    .sum::<u64>(),
            query,
            params,
            records,
            columns: columns.map(Rc::new),
            num_records,
            last_accessed: std::time::Instant::now(),
        }
    }

    pub(super) fn query(&self) -> &str {
        &self.query
    }

    pub(super) fn params(&self) -> &[Literal] {
        &self.params
    }

    pub(super) fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }

    pub(super) fn last_accessed(&self) -> Instant {
        self.last_accessed
    }

    pub(super) fn update_last_accessed(&mut self) {
        self.last_accessed = std::time::Instant::now();
    }

    pub(super) fn set_num_records(&mut self, value: u64) {
        self.num_records = Some(value);
    }

    pub(super) fn set_columns_if_not_set_yet(&mut self, columns: Vec<String>) {
        if self.columns.is_none() {
            self.total_size_bytes += columns.iter().map(|c| into::<_, u64>(c.capacity())).sum::<u64>();
            self.columns = Some(columns.into());
        }
    }

    pub(super) fn insert(&mut self, offset: u64, record: &[Vec<u8>]) {
        let buf = rmp_serde::encode::to_vec(&record).expect("Failed to encode a cache into a msgpack.");
        self.total_size_bytes += into::<_, u64>(std::mem::size_of::<Vec<u8>>()) + into::<_, u64>(buf.capacity());
        self.records.insert(offset, buf);
    }

    /// Returns the ending offset of `OFFSET ? LIMIT ?`. The returned value is always greater than or equal to the `offset`.
    pub(super) fn add_limit_to_offset(&self, offset: u64, limit: u64) -> u64 {
        if let Some(num_records) = self.num_records {
            (offset + limit).min(num_records).max(offset)
        } else {
            offset + limit
        }
    }

    pub(super) fn has_range(&self, offset: u64, limit: u64) -> bool {
        if self.columns.is_none() {
            return false; // this check is needed to unwrap() columns in get_range()
        }
        let end = self.add_limit_to_offset(offset, limit);
        (offset..end).all(|row| self.records.contains_key(&row))
    }

    pub(super) fn get_range(&self, offset: u64, limit: u64) -> Option<Records> {
        if !self.has_range(offset, limit) {
            return None;
        }
        let columns = Rc::clone(self.columns.as_ref().unwrap()); // columns should be Some when has_range() == true
        let end = self.add_limit_to_offset(offset, limit);

        // Decode msgpack
        let records_unpacked: Vec<Vec<Vec<u8>>> = (offset..end)
            .map(|row| rmp_serde::decode::from_slice(&self.records[&row]).expect("Failed to decode a cached msgpack"))
            .collect::<Vec<_>>();

        // Transpose
        let mut col_buf = vec![vec![]; columns.len()];
        for (col, col_buf_i) in col_buf.iter_mut().enumerate() {
            for (row, _) in (offset..end).enumerate() {
                col_buf_i.extend(&records_unpacked[row][col]);
            }
        }

        Some(Records::new(
            col_buf,
            end.checked_sub(offset).unwrap().try_into().unwrap(),
            columns,
        ))
    }
}
