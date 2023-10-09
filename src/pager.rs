use std::{rc::Rc, time::Duration};

use crate::{
    error::Error,
    literal::Literal,
    sqlite3_driver::{write_value_ref_into_msgpack, InvalidUTF8},
};

pub mod cache {
    use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Instant};

    use crate::literal::Literal;

    use super::Records;

    #[derive(Debug)]
    pub struct PagerCacheEntry {
        pub query: String,
        pub params: Vec<Literal>,
        pub records: HashMap<i64, Vec<u8>>,
        pub columns: Option<Rc<Vec<String>>>,
        /// None means unknown
        pub num_records: Option<i64>,
        pub last_accessed: Instant,

        // The approximate size (stack size + heap size) of this struct in bytes.
        total_size_bytes: u64,
    }

    impl PagerCacheEntry {
        pub fn new(
            query: String,
            params: Vec<Literal>,
            records: HashMap<i64, Vec<u8>>,
            columns: Option<Vec<String>>,
            num_records: Option<i64>,
        ) -> Self {
            Self {
                total_size_bytes: std::mem::size_of::<Self>() as u64
                    + query.capacity() as u64
                    + params
                        .iter()
                        .map(|p| {
                            std::mem::size_of_val(p) as u64
                                + match p {
                                    Literal::Bool(_) | Literal::F64(_) | Literal::I64(_) | Literal::Nil => 0,
                                    Literal::Blob(b) => std::mem::size_of_val(b) + b.0.capacity(),
                                    Literal::String(s) => s.capacity(),
                                } as u64
                        })
                        .sum::<u64>(),
                query,
                params,
                records,
                columns: columns.map(|v| Rc::new(v)),
                num_records,
                last_accessed: std::time::Instant::now(),
            }
        }

        pub fn total_size_bytes(&self) -> u64 {
            self.total_size_bytes
        }

        pub fn set_columns_if_not_set_yet<T: Into<Vec<String>>>(&mut self, columns: T) {
            if self.columns.is_none() {
                let columns: Vec<String> = columns.into();
                self.total_size_bytes += columns.iter().map(|c| c.capacity() as u64).sum::<u64>();
                self.columns = Some(columns.into());
            }
        }

        pub fn insert(&mut self, offset: i64, record: &[Vec<u8>]) {
            let buf = rmp_serde::encode::to_vec(&record).expect("Failed to encode a cache into a msgpack.");
            self.total_size_bytes += std::mem::size_of::<Vec<u8>>() as u64 + buf.capacity() as u64;
            self.records.insert(offset, buf);
        }

        fn add_limit_to_offset(&self, offset: i64, limit: i64) -> i64 {
            if let Some(num_records) = self.num_records {
                // Clip `LIMIT ? OFFSET ?` at `num_records` if the number of records is known.
                (offset + limit).min(num_records)
            } else {
                offset + limit
            }
        }

        pub fn has_range(&self, offset: i64, limit: i64) -> bool {
            if self.columns.is_none() {
                return false; // this check is needed to unwrap() columns in get_range()
            }
            let end = self.add_limit_to_offset(offset, limit);
            return (offset..end).all(|row| self.records.contains_key(&row));
        }

        pub fn get_range(&self, offset: i64, limit: i64) -> Option<Records> {
            if !self.has_range(offset, limit) {
                return None;
            }
            let columns = Rc::clone(self.columns.as_ref().unwrap()); // columns should be Some when has_range() == true
            let end = self.add_limit_to_offset(offset, limit);

            // Decode msgpack
            let records_unpacked: Vec<Vec<Vec<u8>>> = (offset..end)
                .into_iter()
                .map(|row| {
                    rmp_serde::decode::from_slice(&self.records[&row]).expect("Failed to decode a cached msgpack")
                })
                .collect::<Vec<_>>();

            // Transpose
            let mut col_buf = vec![vec![]; columns.len()];
            for col in 0..columns.len() {
                for (row, _) in (offset..end).enumerate() {
                    col_buf[col].extend(&records_unpacked[row][col]);
                }
            }

            return Some(Records {
                col_buf,
                n_rows: end - offset,
                columns,
            });
        }
    }

    #[derive(Debug)]
    pub struct PagerCache {
        cache: Vec<Rc<RefCell<PagerCacheEntry>>>,
    }

    impl PagerCache {
        pub fn new() -> Self {
            Self { cache: vec![] }
        }

        /// Returns the cache entry that is bound to (query, params).
        /// Inserts an entry if it does not exist.
        pub fn entry(&mut self, query: &str, params: &[Literal]) -> Rc<RefCell<PagerCacheEntry>> {
            let params = &params[0..(params.len() - 2)];
            for entry in &self.cache {
                {
                    let entry = entry.borrow();
                    if entry.query != query || entry.params != params {
                        continue;
                    }
                }
                entry.borrow_mut().last_accessed = std::time::Instant::now();
                return Rc::clone(entry);
            }
            self.cache.push(Rc::new(RefCell::new(PagerCacheEntry::new(
                query.to_owned(),
                params.to_owned(),
                HashMap::new(),
                None,
                None,
            ))));
            Rc::clone(self.cache.last().unwrap())
        }

        pub fn total_size_bytes(&self) -> u64 {
            self.cache.iter().map(|e| e.borrow().total_size_bytes).sum::<u64>()
        }

        pub fn clear(&mut self) {
            self.cache.clear();
        }

        pub fn dequeue(&mut self) {
            if let Some((index, _)) = self
                .cache
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.borrow().last_accessed)
            {
                self.cache.remove(index);
            }
        }
    }
}

/// Given a query that ends with "LIMIT ? OFFSET ?",
#[derive(Debug)]
pub struct Pager {
    cache: cache::PagerCache,
    data_version: Option<i64>,
    pub config: PagerConfig,

    #[cfg(test)]
    pub cache_hit_count: usize,

    #[cfg(test)]
    pub cache_clear_count: usize,

    #[cfg(test)]
    pub dequeue_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PagerConfig {
    pub slow_query_threshold: Duration,
    pub per_query_cache_limit_bytes: u64,
    pub cache_time_limit_relative_to_queried_range: f64,
    pub cache_limit_bytes: u64,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            slow_query_threshold: Duration::from_millis(500),
            per_query_cache_limit_bytes: /* 1MB */ 1024 * 1024,
            cache_time_limit_relative_to_queried_range: 0.2,
            cache_limit_bytes: /* 64MB */ 64 * 1024 * 1024,
        }
    }
}

fn pragma_data_version(conn: &rusqlite::Connection) -> Result<i64, Error> {
    conn.pragma_query_value(None, "data_version", |row| row.get::<_, i64>(0))
        .or_else(|err| Error::new_query_error(err, "PRAGMA data_version", &[]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Records {
    pub col_buf: Vec<Vec<u8>>,
    pub n_rows: i64,
    pub columns: Rc<Vec<String>>,
}

impl Pager {
    pub fn new() -> Self {
        Self {
            cache: cache::PagerCache::new(),
            data_version: None,
            config: PagerConfig::default(),
            #[cfg(test)]
            cache_hit_count: 0,
            #[cfg(test)]
            cache_clear_count: 0,
            #[cfg(test)]
            dequeue_count: 0,
        }
    }

    #[cfg(test)]
    pub fn data_version(&self) -> Option<i64> {
        self.data_version
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
        #[cfg(test)]
        {
            self.cache_clear_count += 1;
        }
    }

    pub fn total_cache_size_bytes(&self) -> u64 {
        self.cache.total_size_bytes()
    }

    pub fn query<F: FnMut(InvalidUTF8) -> ()>(
        &mut self,
        conn: &mut rusqlite::Connection,
        query: &str,
        params: &[Literal],
        mut on_invalid_utf8: F,
    ) -> std::result::Result<Option<Records>, Error> {
        let mut params = params.to_vec();

        let tx = conn
            .transaction()
            .or_else(|err| Error::new_query_error(err, "BEGIN;", &[]))?;
        let data_version = Some(pragma_data_version(&tx)?);
        if self.data_version != data_version {
            self.clear_cache();
            self.data_version = data_version;
        } else {
            // TODO: use disk
            // TODO: dequeue in an entry
            while self.cache.total_size_bytes() > self.config.cache_limit_bytes {
                self.cache.dequeue();
                #[cfg(test)]
                {
                    self.dequeue_count += 1;
                }
            }
        }

        let len = params.len();
        if !query.ends_with("LIMIT ? OFFSET ?") || len < 2 {
            return Ok(None);
        }

        let (Literal::I64(limit), Literal::I64(offset)) = (&params[len - 2], &params[len - 1]) else {
            return Ok(None);
        };
        let (limit, offset) = (*limit, *offset);

        let cache_entry = self.cache.entry(query, &params);
        let mut cache_entry = cache_entry.borrow_mut();

        // cache hit
        if let Some(records) = cache_entry.get_range(offset, limit) {
            #[cfg(test)]
            {
                self.cache_hit_count += 1;
            }
            return Ok(Some(records));
        }

        // Add margins before and after the queried area
        let limit_with_margin = limit + 100000;
        params[len - 2] = Literal::I64(limit_with_margin);
        let offset_with_margin = (offset - /* disabled */0).max(0);
        params[len - 1] = Literal::I64(offset_with_margin);

        // Forward run: Fetch the queried area and cache records after that
        let mut col_buf: Vec<Vec<u8>>;
        let mut n_rows = 0;
        let columns: Vec<String>;
        let mut end_margin_size = 0;
        {
            // Prepare
            let mut stmt = tx.prepare(query).unwrap();

            // Bind parameters
            for (i, param) in params.iter().enumerate() {
                stmt.raw_bind_parameter(i + 1, param)
                    .or_else(|err| Error::new_query_error(err, query, &params))?;
            }

            // List columns
            columns = stmt
                .column_names()
                .into_iter()
                .map(|v| v.to_owned())
                .collect::<Vec<_>>();
            col_buf = vec![vec![]; columns.len()];

            let cache_size_start = cache_entry.total_size_bytes();
            cache_entry.set_columns_if_not_set_yet::<&[String]>(&columns);

            // Fetch records
            let mut current_offset = offset_with_margin;
            let mut rows = stmt.raw_query();
            let timer = std::time::Instant::now();
            let mut elapsed_until_end_margin = None;
            loop {
                match rows.next() {
                    Ok(Some(row)) => {
                        let is_start_margin = current_offset < offset;
                        let is_end_margin = offset + limit <= current_offset;
                        let is_margin = is_start_margin || is_end_margin;

                        if is_end_margin {
                            if elapsed_until_end_margin.is_none() {
                                elapsed_until_end_margin = Some(timer.elapsed());
                            }
                            let elapsed_until_end_margin = elapsed_until_end_margin.unwrap();

                            if !(
                                // The query is slow and
                                elapsed_until_end_margin >= self.config.slow_query_threshold &&
                                // sqlite3_step()s in the end margin are fast and
                                (timer.elapsed() - elapsed_until_end_margin).div_f64(self.config.cache_time_limit_relative_to_queried_range) < elapsed_until_end_margin &&
                                // record sizes are small
                                cache_entry.total_size_bytes() - cache_size_start < self.config.per_query_cache_limit_bytes / 2
                            ) {
                                break;
                            }

                            end_margin_size += 1;
                        }

                        let mut cache_record = vec![];
                        for i in 0..columns.len() {
                            let mut w = vec![];
                            write_value_ref_into_msgpack(&mut w, row.get_ref_unwrap(i), &mut on_invalid_utf8)
                                .expect("Failed to write msgpack");
                            if !is_margin {
                                col_buf[i].extend(&w);
                            }

                            cache_record.push(w);
                        }
                        cache_entry.insert(current_offset, &cache_record);

                        if !is_margin {
                            n_rows += 1;
                        }
                        current_offset += 1;
                    }
                    Ok(None) => {
                        if current_offset < offset_with_margin + limit_with_margin {
                            cache_entry.num_records = Some(current_offset);
                        }
                        break;
                    }
                    Err(err) => Error::new_query_error(err, query, &params)?,
                }
            }
        }

        // Backward run: cache `end_margin_size` records before the queried area
        // TODO: Send this work to another thread
        if end_margin_size > 0 {
            let backward_offset = (offset - end_margin_size).max(0);
            let backward_limit = offset - backward_offset;
            params[len - 2] = Literal::I64(backward_limit);
            params[len - 1] = Literal::I64(backward_offset);
            let mut current_offset = backward_offset;
            if !cache_entry.has_range(backward_offset, backward_limit) {
                // Prepare
                let mut stmt = tx.prepare(query).unwrap();

                // Bind parameters
                for (i, param) in params.iter().enumerate() {
                    stmt.raw_bind_parameter(i + 1, param)
                        .or_else(|err| Error::new_query_error(err, query, &params))?;
                }

                // Fetch records
                let mut rows = stmt.raw_query();

                loop {
                    match rows.next() {
                        Ok(Some(row)) => {
                            let mut cache_record = vec![];
                            for i in 0..columns.len() {
                                let mut w = vec![];
                                write_value_ref_into_msgpack(&mut w, row.get_ref_unwrap(i), &mut on_invalid_utf8)
                                    .expect("Failed to write msgpack");
                                cache_record.push(w);
                            }
                            cache_entry.insert(current_offset, &cache_record);

                            current_offset += 1;
                        }
                        Ok(None) => {
                            break;
                        }
                        Err(err) => Error::new_query_error(err, query, &params)?,
                    }
                }
            }
        }

        Ok(Some(Records {
            col_buf,
            n_rows,
            columns: Rc::new(columns),
        }))
    }
}
