use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::cli_value::CLIValue;
use crate::msgpack::MessagePackArray;
use crate::util::into;

#[derive(Clone, Debug, PartialEq, Eq, derive_new::new)]
pub struct Records {
    col_buf: Vec<MessagePackArray>,
    // The max number of elements in an array in msgpack is u32::MAX.
    columns: Rc<Vec<String>>,
}

impl Records {
    pub fn col_buf(&self) -> &[MessagePackArray] {
        &self.col_buf
    }
    pub fn columns(&self) -> Rc<Vec<String>> {
        Rc::clone(&self.columns)
    }
}

/// Stores the last result of a query.
pub(super) struct PagerCacheEntry {
    query: String,
    params: Vec<CLIValue>,

    records: HashMap</* offset */ u64, MessagePackArray>,
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
        params: Vec<CLIValue>,
        records: HashMap<u64, MessagePackArray>,
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
                                CLIValue::Bool(_) | CLIValue::F64(_) | CLIValue::I64(_) | CLIValue::Nil => 0u64,
                                CLIValue::Blob(b) => into::<_, u64>(std::mem::size_of_val(b) + b.0.capacity()),
                                CLIValue::String(s) => into::<_, u64>(s.capacity()),
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

    pub(super) fn params(&self) -> &[CLIValue] {
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

    pub(super) fn insert(&mut self, offset: u64, mut record: MessagePackArray) {
        record.shrink_to_fit();
        self.total_size_bytes += record.memory_usage() as u64;
        self.records.insert(offset, record);
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
        let records_unpacked: Vec<&MessagePackArray> = (offset..end).map(|row| &self.records[&row]).collect::<Vec<_>>();

        // Transpose
        let mut col_buf = Vec::with_capacity(columns.len());
        for _ in 0..columns.len() {
            col_buf.push(MessagePackArray::new());
        }
        for (col, col_buf_i) in col_buf.iter_mut().enumerate() {
            for (row, _) in (offset..end).enumerate() {
                col_buf_i.push_message_pack(records_unpacked[row].get_as_msgpack(col).unwrap().to_owned());
            }
        }

        Some(Records::new(col_buf, columns))
    }
}
