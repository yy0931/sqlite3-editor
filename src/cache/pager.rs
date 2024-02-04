use std::{rc::Rc, time::Duration};

use crate::{
    error::Error,
    literal::Literal,
    sqlite3_driver::{write_value_ref_into_msgpack, InvalidUTF8},
};

use super::{cache_entry::Records, pager_cache::PagerCache};

#[derive(Debug, Clone, PartialEq)]
pub struct PagerConfig {
    pub slow_query_threshold: Duration,
    pub per_query_cache_limit_bytes: u64,
    pub cache_time_limit_relative_to_queried_range: f64,
    pub cache_limit_bytes: u64,

    /// The number of records to prefetch before the start of the queried range.
    pub margin_start: u64,
    /// The number of records to prefetch after the end of the queried range.
    pub margin_end: u64,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            slow_query_threshold: Duration::from_millis(500),
            per_query_cache_limit_bytes: /* 8MB */ 8 * 1024 * 1024,
            cache_time_limit_relative_to_queried_range: 0.2,
            cache_limit_bytes: /* 64MB */ 64 * 1024 * 1024,
            margin_start: 0,
            margin_end: 100000,
        }
    }
}

/// Given a query that ends with "LIMIT ? OFFSET ?",
pub struct Pager {
    cache: PagerCache,
    data_version: Option<i64>,
    pub config: PagerConfig,

    #[cfg(test)]
    pub cache_hit_count: usize,

    #[cfg(test)]
    pub cache_clear_count: usize,

    #[cfg(test)]
    pub dequeue_count: usize,
}

impl Pager {
    pub fn new() -> Self {
        Self {
            cache: PagerCache::new(),
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

    pub fn query<F: FnMut(InvalidUTF8)>(
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
        let (Ok(limit), Ok(offset)): (Result<u64, _>, Result<u64, _>) = ((*limit).try_into(), (*offset).try_into())
        else {
            // Negative limits and negative offsets are not supported
            return Ok(None);
        };

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
        let limit_with_margin = limit + self.config.margin_start.min(offset) + self.config.margin_end;
        params[len - 2] = Literal::I64(limit_with_margin.try_into().unwrap());
        let offset_with_margin = offset.saturating_sub(self.config.margin_start);
        params[len - 1] = Literal::I64(offset_with_margin.try_into().unwrap());

        // Forward run: Fetch the queried area and cache records after that
        let mut col_buf: Vec<Vec<u8>>;
        let mut n_rows: u32 = 0;
        let columns: Vec<String>;
        let mut end_margin_size = 0;
        {
            // Prepare
            let mut stmt = tx
                .prepare(query)
                .or_else(|err| Error::new_query_error(err, query, &params))?;

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

            let cache_size_prev = cache_entry.total_size_bytes();
            cache_entry.set_columns_if_not_set_yet(columns.clone());

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
                                cache_entry.total_size_bytes().saturating_sub(cache_size_prev) < self.config.per_query_cache_limit_bytes / 2
                            ) {
                                break;
                            }

                            end_margin_size += 1;
                        }

                        let mut cache_record = vec![];
                        for (i, col_buf_i) in col_buf.iter_mut().enumerate() {
                            let mut w = vec![];
                            write_value_ref_into_msgpack(&mut w, row.get_ref_unwrap(i), &mut on_invalid_utf8)
                                .expect("Failed to write msgpack");
                            if !is_margin {
                                col_buf_i.extend(&w);
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
                            cache_entry.set_num_records(current_offset);
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
            let backward_offset = offset.saturating_sub(end_margin_size);
            let backward_limit = offset.saturating_sub(backward_offset);
            params[len - 2] = Literal::I64(backward_limit.try_into().unwrap());
            params[len - 1] = Literal::I64(backward_offset.try_into().unwrap());
            let mut current_offset = backward_offset;
            if !cache_entry.has_range(backward_offset, backward_limit) {
                // Prepare
                let mut stmt = tx
                    .prepare(query)
                    .or_else(|err| Error::new_query_error(err, query, &params))?;

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

        Ok(Some(Records::new(col_buf, n_rows, Rc::new(columns))))
    }
}

fn pragma_data_version(conn: &rusqlite::Connection) -> Result<i64, Error> {
    conn.pragma_query_value(None, "data_version", |row| row.get::<_, i64>(0))
        .or_else(|err| Error::new_query_error(err, "PRAGMA data_version", &[]))
}
